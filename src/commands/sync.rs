use crate::application::services::sync_executor::{SyncExecuteOptions, execute_sync_actions};
use crate::application::services::sync_service::{
    SyncDiffOptions, SyncScanOptions, compute_diff, format_action_line, parse_sync_endpoint,
    resolve_sync_direction, scan_cloud_personal, scan_local,
};
use crate::client::StorageType;
use crate::domain::{SyncDirection, SyncEndpoint, SyncSummary};
use crate::{debug, step, success, warn};
use std::fmt;
use std::io::IsTerminal;

#[derive(Debug, Clone)]
pub struct SyncArgs {
    pub src: String,
    pub dest: String,
    pub recursive: bool,
    pub dry_run: bool,
    pub delete: bool,
    pub checksum: bool,
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
        checksum: args.checksum,
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

    if args.checksum {
        let cloud_entries = match direction {
            SyncDirection::LocalToCloud => &target,
            SyncDirection::CloudToLocal => &source,
        };
        let missing_count = cloud_entries
            .iter()
            .filter(|e| e.checksum.is_none())
            .count();
        if missing_count > 0 {
            warn!(
                "{} 个云端文件缺少校验和，将回退到大小+时间比对",
                missing_count
            );
        }
    }

    let actions = compute_diff(
        &source,
        &target,
        SyncDiffOptions {
            direction,
            delete: args.delete,
            checksum: args.checksum,
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
    success!(
        "同步完成: {} 个文件传输, {} 个目录创建, {} 个删除, {} 个跳过, {} 个失败",
        summary.transferred,
        summary.created_dirs,
        summary.deleted,
        summary.skipped,
        summary.failed
    );
    debug!(
        "transferred {} bytes in {} files",
        summary.bytes, summary.transferred
    );
}
