use crate::application::services::sync_executor::{SyncExecuteOptions, execute_sync_actions};
use crate::application::services::sync_service::{
    SyncDiffOptions, SyncScanOptions, compute_diff, format_action_line, parse_sync_endpoint,
    resolve_sync_direction, scan_cloud_personal, scan_local,
};
use crate::client::StorageType;
use crate::domain::{SyncDirection, SyncEndpoint, SyncSummary};
use crate::{debug, step, success};
use std::fmt;
use std::io::IsTerminal;

#[derive(Debug, Clone)]
pub struct SyncArgs {
    pub src: String,
    pub dest: String,
    pub recursive: bool,
    pub dry_run: bool,
    pub delete: bool,
    pub exclude: Vec<String>,
    pub jobs: usize,
}

#[derive(Debug)]
pub struct CommandExit {
    code: i32,
    message: String,
}

impl CommandExit {
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn code(&self) -> i32 {
        self.code
    }
}

impl fmt::Display for CommandExit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for CommandExit {}

pub async fn execute(args: SyncArgs) -> anyhow::Result<()> {
    let direction = resolve_sync_direction(&args.src, &args.dest)
        .map_err(|err| CommandExit::new(2, err.to_string()))?;

    let config = crate::commands::dispatch::load_config()
        .map_err(|err| CommandExit::new(2, err.to_string()))?;

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

    print_summary(&summary);

    if summary.failed > 0 {
        return Err(CommandExit::new(1, format!("{} 个文件同步失败", summary.failed)).into());
    }

    Ok(())
}

fn print_summary(summary: &SyncSummary) {
    let mut parts = Vec::new();
    if summary.transferred > 0 {
        parts.push(format!("{} 个文件传输", summary.transferred));
    }
    if summary.created_dirs > 0 {
        parts.push(format!("{} 个目录创建", summary.created_dirs));
    }
    if summary.deleted > 0 {
        parts.push(format!("{} 个删除", summary.deleted));
    }
    if summary.skipped > 0 {
        parts.push(format!("{} 个跳过", summary.skipped));
    }
    if summary.failed > 0 {
        parts.push(format!("{} 个失败", summary.failed));
    }
    if parts.is_empty() {
        success!("同步完成: 无变化");
    } else {
        success!("同步完成: {}", parts.join(", "));
    }
}
