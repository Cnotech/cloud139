use crate::commands::CommandExit;
use crate::domain::StorageType;
use crate::domain::{SyncAction, SyncDirection, SyncEndpoint, SyncEntryKind};
use crate::services::sync_executor::{
    SyncExecuteOptions, SyncReport, SyncVerifyOptions, execute_sync_actions, execute_verified_sync,
};
use crate::services::sync_service::{
    SyncDiffOptions, SyncScanOptions, compute_diff, compute_diff_with_stats, format_action_line,
    parse_sync_endpoint, resolve_sync_direction, scan_cloud_personal, scan_local,
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

    #[arg(
        short = 'j',
        long,
        value_parser = parse_jobs,
        help = "并发传输数量上限（默认 4；--verify 下固定为 1）"
    )]
    pub jobs: Option<usize>,

    #[arg(
        long,
        help = "校验远端内容（仅 本地→云端）：size + contentHash 不一致时安全替换"
    )]
    pub verify: bool,

    #[arg(
        long,
        help = "输出 JSON 汇总（需与 --verify 一起用；stdout 只有一行 JSON）"
    )]
    pub json: bool,
}

/// 参数校验：全部归 exit 2（参数错误），与对象传输失败（1）区分开。
pub fn validate_args(args: &SyncArgs) -> Result<(), CommandExit> {
    if args.json && !args.verify {
        return Err(CommandExit::new(2, "--json 必须与 --verify 一起使用"));
    }
    if args.verify && matches!(args.jobs, Some(jobs) if jobs > 1) {
        return Err(CommandExit::new(
            2,
            "--verify 要求 -j 1：并发会漏对象、破坏校验前提",
        ));
    }
    Ok(())
}

pub async fn execute(args: SyncArgs) -> anyhow::Result<()> {
    if args.json {
        crate::utils::logger::set_json_mode(true);
    }
    let json = args.json;
    let src = args.src.clone();
    // seconds 从命令开始算，含扫描；不能只算执行阶段。
    let started = std::time::Instant::now();
    match run(args).await {
        Ok(Some(mut report)) => {
            report.seconds = started.elapsed().as_secs_f64();
            if json {
                crate::print!("{}", serde_json::to_string(&report)?);
            } else {
                print_verified_report(&report);
            }
            if !report.failures.is_empty() {
                return Err(CommandExit::new(
                    1,
                    format!("{} 个对象同步失败", report.failures.len()),
                )
                .into());
            }
            Ok(())
        }
        Ok(None) => Ok(()),
        Err(err) => {
            if json {
                crate::print!("{}", fatal_report_json(&src, &format!("{err:#}")));
            }
            if err.is::<CommandExit>() {
                Err(err)
            } else {
                Err(CommandExit::new(1, format!("{err:#}")).into())
            }
        }
    }
}

fn print_verified_report(report: &SyncReport) {
    crate::info!(
        "同步完成: 对象 {}，上传 {}，跳过 {}，删除 {}，缺哈希 {}，失败 {}，耗时 {:.1}s",
        report.objects,
        report.uploaded,
        report.skipped,
        report.deleted,
        report.hash_missing,
        report.failures.len(),
        report.seconds
    );
    for failure in &report.failures {
        crate::error!(
            "失败 {}: {}（重试 {}）",
            failure.path,
            failure.error,
            failure.retries
        );
    }
}

/// 致命错误（含 exit 2 的参数错误）也要给调用方一份可解析的七字段 JSON。
fn fatal_report_json(src: &str, message: &str) -> String {
    serde_json::json!({
        "objects": 0,
        "uploaded": 0,
        "skipped": 0,
        "deleted": 0,
        "hashMissing": 0,
        "seconds": 0.0,
        "failures": [{
            "path": src,
            "error": message,
            "retries": 0
        }]
    })
    .to_string()
}

async fn run(args: SyncArgs) -> anyhow::Result<Option<SyncReport>> {
    validate_args(&args)?;

    let direction = resolve_sync_direction(&args.src, &args.dest)
        .map_err(|err| CommandExit::new(2, err.to_string()))?;

    if args.verify && direction != SyncDirection::LocalToCloud {
        return Err(CommandExit::new(2, "--verify 仅支持 本地 → 云端").into());
    }

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
            if args.verify && args.delete {
                let file_count = source
                    .iter()
                    .filter(|entry| entry.kind == SyncEntryKind::File)
                    .count();
                if file_count == 0 {
                    return Err(CommandExit::new(
                        2,
                        "源目录为空，拒绝执行 --delete（不做任何删除）",
                    )
                    .into());
                }
            }
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

    if args.verify {
        let (actions, stats) = compute_diff_with_stats(
            &source,
            &target,
            SyncDiffOptions {
                direction,
                delete: args.delete,
                local_root,
                cloud_root,
                verify: true,
            },
        );
        if args.dry_run && !args.json {
            for action in actions
                .iter()
                .filter(|action| !matches!(action, SyncAction::Skip { .. }))
            {
                step!("{}", format_action_line(action, true));
            }
        }
        let report = execute_verified_sync(
            &config,
            actions,
            stats,
            SyncVerifyOptions {
                dry_run: args.dry_run,
                delete: args.delete,
                progress: is_tty && !args.json,
                print_actions: !is_tty && !args.dry_run && !args.json,
            },
        )
        .await?;
        return Ok(Some(report));
    }

    let actions = compute_diff(
        &source,
        &target,
        SyncDiffOptions {
            direction,
            delete: args.delete,
            local_root,
            cloud_root,
            verify: false,
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
            jobs: args.jobs.unwrap_or(4),
            progress: is_tty,
            print_actions: !is_tty && !args.dry_run,
        },
    )
    .await?;

    crate::presentation::print_summary(&summary);

    if summary.failed > 0 {
        return Err(CommandExit::new(1, format!("{} 个文件同步失败", summary.failed)).into());
    }

    Ok(None)
}
