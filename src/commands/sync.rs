use crate::commands::CommandExit;
use crate::domain::{SyncAction, SyncDirection, SyncEndpoint, SyncEntryKind};
use crate::services::sync_executor::{MAX_JOBS, SyncExecuteOptions, execute_scanned_sync};
use crate::services::sync_service::{
    SyncDiffOptions, SyncScanOptions, format_action_line, parse_sync_endpoint,
    scan_cloud_download_source, scan_cloud_upload_target, scan_local_target, scan_upload_source,
    sync_direction,
};
use crate::services::sync_verify::execute_verified_sync;
use crate::step;
use clap::Parser;
use std::io::IsTerminal;

fn parse_jobs(value: &str) -> Result<usize, String> {
    let jobs = value
        .parse::<usize>()
        .map_err(|_| "并发数量必须为正整数".to_string())?;
    if !(1..=MAX_JOBS).contains(&jobs) {
        return Err(format!("并发数量必须介于 1 和 {MAX_JOBS} 之间"));
    }
    Ok(jobs)
}

#[derive(Parser, Debug, Clone)]
#[command(
    after_help = "个人盘统一使用 PC 客户端标识，上传大小由服务器按账号权益校验。同步上传成功后才替换同名条目；有失败项返回退出码 1，-n 仅预览计划。"
)]
pub struct SyncArgs {
    #[arg(help = "源路径，本地路径或 cloud:/remote/path")]
    pub src: String,

    #[arg(help = "目标路径，本地路径或 cloud:/remote/path")]
    pub dest: String,

    #[arg(short = 'r', long, help = "递归同步子目录，空目录也会同步")]
    pub recursive: bool,

    #[arg(short = 'n', long, help = "演习模式，只输出操作计划")]
    pub dry_run: bool,

    #[arg(long, help = "删除目标中源没有的文件或空目录")]
    pub delete: bool,

    #[arg(long, value_name = "PAT", help = "排除匹配的路径，可多次指定")]
    pub exclude: Vec<String>,

    #[arg(short = 'j', long, default_value_t = 4, default_value_if("verify", "true", Some("1")),
        value_parser = parse_jobs, help = "并发传输数量上限，默认 4；--verify 固定为 1")]
    pub jobs: usize,

    #[arg(long, help = "校验内容并修复不一致的文件（仅本地到个人盘）")]
    pub verify: bool,

    #[arg(long, requires = "verify", help = "输出一行 JSON 校验汇总")]
    pub json: bool,
}

pub async fn execute(args: SyncArgs) -> anyhow::Result<()> {
    let src = parse_sync_endpoint(&args.src);
    let dest = parse_sync_endpoint(&args.dest);
    let direction =
        sync_direction(&src, &dest).map_err(|err| CommandExit::new(2, err.to_string()))?;
    if args.verify && direction != SyncDirection::LocalToCloud {
        return Err(CommandExit::new(2, "--verify 仅支持本地到个人盘").into());
    }
    if args.verify && args.jobs != 1 {
        return Err(CommandExit::new(2, "--verify 只允许 -j 1").into());
    }

    let mut config = crate::config::Config::load_personal().map_err(|error| match error {
        crate::client::ClientError::UnsupportedStorage => anyhow::Error::from(error),
        _ => CommandExit::new(2, error.to_string()).into(),
    })?;

    let scan_options = SyncScanOptions {
        recursive: args.recursive,
        exclude: args.exclude.clone(),
    };

    let progress = !args.json && std::io::stderr().is_terminal();
    let execution = SyncExecuteOptions {
        dry_run: args.dry_run,
        jobs: args.jobs,
        progress,
        print_actions: !progress && !args.dry_run,
    };

    let diff_options = |local_root, cloud_root| SyncDiffOptions {
        direction,
        delete: args.delete,
        local_root,
        cloud_root,
    };
    let (actions, cloud, local) = match (src, dest) {
        (SyncEndpoint::Local(local_root), SyncEndpoint::Cloud(cloud_root)) => {
            let (local_root, source) = scan_upload_source(local_root, &scan_options, args.delete)
                .map_err(|err| CommandExit::new(2, err.to_string()))?;
            let objects = source
                .iter()
                .filter(|entry| entry.kind == SyncEntryKind::File)
                .count();
            if args.verify && args.delete && objects == 0 {
                return Err(CommandExit::new(
                    2,
                    "--verify --delete 要求同步范围内至少有一个源文件",
                )
                .into());
            }
            let target = scan_cloud_upload_target(&config, &cloud_root, scan_options)
                .await
                .map_err(|err| CommandExit::new(2, err.to_string()))?;
            config.personal_cloud_host = Some(target.host.clone());
            let cloud_root = target.root.clone();
            step!("正在计算同步差异...");
            let actions = target.compute_upload_diff(
                &source,
                diff_options(local_root, cloud_root.clone()),
                args.verify,
            );
            print_plan(&actions, args.dry_run);
            if args.verify {
                let report = execute_verified_sync(
                    &config,
                    &target,
                    &cloud_root,
                    &actions,
                    objects,
                    execution,
                )
                .await;
                if args.json {
                    crate::print!("{}", serde_json::to_string(&report)?);
                } else {
                    let label = if args.dry_run {
                        "校验计划"
                    } else {
                        "校验汇总"
                    };
                    crate::info!(
                        "{label}: {} 个源文件，{} 个上传，{} 个跳过，{} 个删除，{} 个缺少可比哈希，{} 个失败",
                        report.objects,
                        report.uploaded,
                        report.skipped,
                        report.deleted,
                        report.hash_missing,
                        report.failures.len(),
                    );
                }
                return finish(args.dry_run, report.failures.len());
            }
            (actions, Some(target), None)
        }
        (SyncEndpoint::Cloud(cloud_root), SyncEndpoint::Local(local_root)) => {
            let source = scan_cloud_download_source(&config, &cloud_root, scan_options.clone())
                .await
                .map_err(|err| CommandExit::new(2, err.to_string()))?;
            config.personal_cloud_host = Some(source.host.clone());
            let target = scan_local_target(&local_root, &scan_options)
                .map_err(|err| CommandExit::new(2, err.to_string()))?;
            target
                .validate_destinations(&source, &local_root)
                .map_err(|err| CommandExit::new(2, err.to_string()))?;
            step!("正在计算同步差异...");
            let actions =
                source.compute_download_diff(&target, diff_options(local_root, cloud_root));
            print_plan(&actions, args.dry_run);
            (actions, Some(source), Some(target))
        }
        _ => return Err(CommandExit::new(2, "无效的同步方向").into()),
    };

    let summary =
        execute_scanned_sync(&config, actions, execution, cloud.as_ref(), local.as_ref()).await?;
    crate::presentation::sync_renderer::print_summary_with_mode(&summary, args.dry_run);
    finish(args.dry_run, summary.failed)
}

fn print_plan(actions: &[SyncAction], dry_run: bool) {
    if dry_run {
        for action in actions
            .iter()
            .filter(|action| !matches!(action, SyncAction::Skip { .. }))
        {
            step!("{}", format_action_line(action, true));
        }
    }
}

fn finish(dry_run: bool, failed: usize) -> anyhow::Result<()> {
    if !dry_run && failed > 0 {
        return Err(CommandExit::new(1, format!("同步未完成: {failed} 个失败")).into());
    }

    Ok(())
}
