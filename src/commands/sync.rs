use crate::commands::CommandExit;
use crate::domain::StorageType;
use crate::domain::{SyncDirection, SyncEndpoint};
use crate::services::sync_executor::{SyncExecuteOptions, execute_sync_actions};
use crate::services::sync_service::{
    SyncDiffOptions, SyncScanOptions, compute_diff, format_action_line, parse_sync_endpoint,
    resolve_sync_direction, scan_cloud_personal, scan_local,
};
use crate::{debug, step};
use clap::Parser;
use std::io::IsTerminal;

fn parse_jobs(value: &str) -> Result<usize, String> {
    let jobs = value
        .parse::<usize>()
        .map_err(|_| "jobs must be a positive integer".to_string())?;
    if jobs == 0 {
        return Err("jobs must be greater than 0".to_string());
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

    #[arg(short = 'n', long, help = "演习模式，只输出操作计划")]
    pub dry_run: bool,

    #[arg(long, help = "删除目标中源没有的文件或空目录")]
    pub delete: bool,

    #[arg(long, value_name = "PAT", help = "排除匹配的路径，可多次指定")]
    pub exclude: Vec<String>,

    #[arg(short = 'j', long, default_value = "4", value_parser = parse_jobs, help = "并发传输数量上限")]
    pub jobs: usize,
}

pub async fn execute(args: SyncArgs) -> anyhow::Result<()> {
    let direction = resolve_sync_direction(&args.src, &args.dest)
        .map_err(|err| CommandExit::new(2, err.to_string()))?;

    let config =
        crate::config::Config::load().map_err(|err| CommandExit::new(2, err.to_string()))?;

    if config.storage_type() != StorageType::PersonalNew {
        return Err(CommandExit::new(2, "sync 当前仅支持个人云").into());
    }

    let src = parse_sync_endpoint(&args.src);
    let dest = parse_sync_endpoint(&args.dest);
    let scan_options = SyncScanOptions {
        recursive: args.recursive,
        exclude: args.exclude.clone(),
    };

    let is_tty = std::io::stderr().is_terminal();

    let (source, target, local_root, cloud_root) = match (direction, src, dest) {
        (
            SyncDirection::LocalToCloud,
            SyncEndpoint::Local(local_root),
            SyncEndpoint::Cloud(cloud_root),
        ) => {
            let source = scan_local(&local_root, scan_options.clone())
                .map_err(|err| CommandExit::new(2, err.to_string()))?;
            let target = scan_cloud_personal(&config, &cloud_root, scan_options)
                .await
                .map_err(|err| CommandExit::new(2, err.to_string()))?;
            (source, target, local_root, cloud_root)
        }
        (
            SyncDirection::CloudToLocal,
            SyncEndpoint::Cloud(cloud_root),
            SyncEndpoint::Local(local_root),
        ) => {
            let source = scan_cloud_personal(&config, &cloud_root, scan_options.clone())
                .await
                .map_err(|err| CommandExit::new(2, err.to_string()))?;
            let target = if local_root.exists() {
                scan_local(&local_root, scan_options)
                    .map_err(|err| CommandExit::new(2, err.to_string()))?
            } else {
                Vec::new()
            };
            (source, target, local_root, cloud_root)
        }
        _ => return Err(CommandExit::new(2, "无效的同步方向").into()),
    };

    debug!(
        "sync: 方向={:?}, 源条目={}, 目标条目={}",
        direction,
        source.len(),
        target.len()
    );

    step!("正在计算同步差异...");
    let actions = compute_diff(
        &source,
        &target,
        SyncDiffOptions {
            direction,
            delete: args.delete,
            local_root,
            cloud_root,
        },
    );

    if args.dry_run {
        for action in actions
            .iter()
            .filter(|action| !matches!(action, crate::domain::SyncAction::Skip { .. }))
        {
            step!("{}", format_action_line(action, true));
        }
    }

    let summary = execute_sync_actions(
        &config,
        actions,
        SyncExecuteOptions {
            dry_run: args.dry_run,
            jobs: args.jobs,
            progress: is_tty,
            print_actions: !is_tty && !args.dry_run,
        },
    )
    .await?;

    crate::presentation::print_summary(&summary);

    if summary.failed > 0 {
        return Err(CommandExit::new(1, format!("{} 个文件同步失败", summary.failed)).into());
    }

    Ok(())
}
