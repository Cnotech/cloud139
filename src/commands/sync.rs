use crate::client::ClientError;
use crate::commands::CommandExit;
use crate::config::Config;
use crate::domain::{SyncAction, SyncDirection, SyncEndpoint, SyncEntryKind};
use crate::presentation::sync_renderer::{print_json_report, print_summary};
use crate::services::sync_executor::{MAX_JOBS, SyncExecuteOptions, execute_scanned_sync};
use crate::services::sync_report::SyncReport;
use crate::services::sync_service::{
    SyncDiffOptions, SyncScanOptions, format_action_line, parse_sync_endpoint, scan_cloud,
    scan_local_target, scan_upload_source, sync_direction,
};
use crate::services::sync_verify::execute_verified_sync;
use crate::step;
use clap::Parser;
use std::io::IsTerminal;
use std::time::Instant;

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
pub struct SyncArgs {
    #[arg(help = "源路径，本地路径或 cloud:/remote/path")]
    pub src: String,

    #[arg(help = "目标路径，本地路径或 cloud:/remote/path")]
    pub dest: String,

    #[arg(short = 'r', long, help = "递归同步子目录，空目录也会同步")]
    pub recursive: bool,

    #[arg(short = 'n', long, help = "只预览同步计划")]
    pub dry_run: bool,

    #[arg(long, help = "删除目标中源没有的文件或空目录")]
    pub delete: bool,

    #[arg(long, value_name = "PAT", help = "排除匹配的路径，可多次指定")]
    pub exclude: Vec<String>,

    #[arg(short = 'j', long, default_value_t = 4, default_value_if("verify", "true", Some("1")),
        value_parser = parse_jobs, help = format!("同时传输的文件数，范围 1 到 {MAX_JOBS}。默认 4，--verify 固定为 1"))]
    pub jobs: usize,

    #[arg(long, help = "校验内容并修复不一致的文件")]
    pub verify: bool,

    #[arg(long, help = "以 JSON 报告本次同步的文件变更")]
    pub json: bool,
}

pub async fn execute(args: SyncArgs) -> anyhow::Result<()> {
    let endpoints = preflight(&args)?;
    let config = Config::load_personal().map_err(config_error)?;
    execute_with_config(args, endpoints, config).await
}

pub(super) struct SyncEndpoints {
    src: SyncEndpoint,
    dest: SyncEndpoint,
    direction: SyncDirection,
}

pub(super) fn preflight(args: &SyncArgs) -> Result<SyncEndpoints, CommandExit> {
    let src = parse_sync_endpoint(&args.src);
    let dest = parse_sync_endpoint(&args.dest);
    let direction =
        sync_direction(&src, &dest).map_err(|err| CommandExit::new(2, err.to_string()))?;
    if args.verify && direction != SyncDirection::LocalToCloud {
        return Err(CommandExit::new(2, "--verify 要求目标路径以 cloud: 开头"));
    }
    if args.verify && args.jobs != 1 {
        return Err(CommandExit::new(2, "--verify 只允许 -j 1"));
    }
    Ok(SyncEndpoints {
        src,
        dest,
        direction,
    })
}

pub(super) fn config_error(error: ClientError) -> anyhow::Error {
    match error {
        ClientError::UnsupportedStorage => error.into(),
        _ => CommandExit::new(2, error.to_string()).into(),
    }
}

pub(super) async fn execute_with_config(
    args: SyncArgs,
    endpoints: SyncEndpoints,
    mut config: Config,
) -> anyhow::Result<()> {
    let SyncEndpoints {
        src,
        dest,
        direction,
    } = endpoints;
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
    let mut report = (args.json && !args.verify).then(|| SyncReport::new(args.dry_run, true));
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
            let target = scan_cloud(&config, &cloud_root, &scan_options, direction)
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
                let mut report = execute_verified_sync(
                    &config,
                    &target,
                    &cloud_root,
                    &actions,
                    objects,
                    execution,
                    args.json,
                )
                .await;
                if args.json {
                    print_json_report(&args.src, &args.dest, &mut report)?;
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
            if let Some(report) = &mut report {
                report.count_source_files(&source, &actions);
            }
            (actions, Some(target), None)
        }
        (SyncEndpoint::Cloud(cloud_root), SyncEndpoint::Local(local_root)) => {
            let source = scan_cloud(&config, &cloud_root, &scan_options, direction)
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
            if let Some(report) = &mut report {
                report.count_source_files(&source.items, &actions);
            }
            print_plan(&actions, args.dry_run);
            (actions, Some(source), Some(target))
        }
        _ => return Err(CommandExit::new(2, "无效的同步方向").into()),
    };

    let started = Instant::now();
    let summary = execute_scanned_sync(
        &config,
        actions,
        execution,
        cloud.as_ref(),
        local.as_ref(),
        report.as_mut(),
    )
    .await?;
    if let Some(mut report) = report {
        report.seconds = started.elapsed().as_secs_f64();
        print_json_report(&args.src, &args.dest, &mut report)?;
    } else {
        print_summary(&summary, args.dry_run);
    }
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
