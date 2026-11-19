use super::progress::{Progress, ProgressFactory};
use super::sync_remote::SyncCloud;
use super::sync_report::SyncReport;
use crate::models::sync_item::{SyncAction, SyncSummary, SyncTarget};
use crate::services::local_scan::LocalSyncScan;
use crate::services::sync_service::{CloudSyncScan, format_action_line};
use crate::utils::path::safe_local;
use anyhow::{Context, Result, ensure};
use futures_util::StreamExt;
use std::path::Path;

// 限制单次同步同时占用的网络连接、打开的文件和哈希任务。
pub const MAX_JOBS: usize = 32;

#[derive(Debug, Clone, Copy)]
pub struct SyncExecuteOptions {
    pub dry_run: bool,
    pub jobs: usize,
    pub print_actions: bool,
}

pub async fn execute_sync_actions(
    config: &crate::config::Config,
    actions: Vec<SyncAction>,
    options: SyncExecuteOptions,
) -> Result<SyncSummary> {
    execute_scanned_sync(config, actions, options, None, None, None, None).await
}

pub(super) async fn execute_scanned_sync(
    config: &crate::config::Config,
    actions: Vec<SyncAction>,
    options: SyncExecuteOptions,
    cloud: Option<&CloudSyncScan>,
    local: Option<&LocalSyncScan>,
    mut report: Option<&mut SyncReport>,
    progress: Option<&ProgressFactory<'_>>,
) -> Result<SyncSummary> {
    config.require_personal()?;
    ensure!(
        (1..=MAX_JOBS).contains(&options.jobs),
        "同步并发数量必须介于 1 和 {MAX_JOBS} 之间"
    );
    if options.dry_run {
        if let Some(report) = report {
            record_plan(report, &actions, cloud, local);
        }
        return Ok(summarize_actions(&actions));
    }

    let remote = SyncCloud::new(config, cloud);
    let mut summary = SyncSummary {
        skipped: count_skips(&actions),
        ..SyncSummary::default()
    };
    let run = |action| {
        let remote = &remote;
        async move {
            let bar = make_action_progress(progress, action);
            let mut file_changed = false;
            let outcome = execute_one_action(
                config,
                action,
                bar.clone(),
                remote,
                cloud,
                local,
                &mut file_changed,
            )
            .await;
            if let Some(bar) = bar {
                bar.finish();
            }
            (action, file_changed, outcome)
        }
    };

    // Directory creation establishes parents before transfers begin. Deletion is last
    // and runs child first so a parent can only be removed after its children.
    let (directories, remaining): (Vec<_>, Vec<_>) = actions
        .iter()
        .filter(|action| !matches!(action, SyncAction::Skip { .. }))
        .partition(|action| matches!(action, SyncAction::CreateDir { .. }));
    let (deletes, transfers): (Vec<_>, Vec<_>) = remaining
        .into_iter()
        .partition(|action| matches!(action, SyncAction::Delete { .. }));
    for action in directories {
        record_result(
            &mut summary,
            report.as_deref_mut(),
            run(action).await,
            options,
            cloud,
        );
    }
    let mut pending =
        futures_util::stream::iter(transfers.into_iter().map(run)).buffer_unordered(options.jobs);
    while let Some(result) = pending.next().await {
        record_result(&mut summary, report.as_deref_mut(), result, options, cloud);
    }
    if summary.failed == 0 {
        for action in deletes {
            record_result(
                &mut summary,
                report.as_deref_mut(),
                run(action).await,
                options,
                cloud,
            );
            if summary.failed > 0 {
                break;
            }
        }
    }
    Ok(summary)
}

fn record_result(
    summary: &mut SyncSummary,
    mut report: Option<&mut SyncReport>,
    (action, file_changed, outcome): (&SyncAction, bool, Result<()>),
    options: SyncExecuteOptions,
    cloud: Option<&CloudSyncScan>,
) {
    if file_changed && let Some(report) = report.as_deref_mut() {
        report.record_file_action(action, cloud);
    }
    if let Err(error) = outcome {
        let path = action_rel_path(action);
        let detail = format!("{error:#}");
        let message = format!("同步失败: {path}: {detail}");
        crate::error!("{message}");
        if let Some(report) = report {
            report.record_failure(path, &error, detail);
        }
        summary.failed += 1;
        return;
    }
    if options.print_actions {
        let message = format_action_line(action, false);
        crate::step!("{message}");
    }
    merge_summary(summary, summary_for_success(action));
}

fn record_plan(
    report: &mut SyncReport,
    actions: &[SyncAction],
    cloud: Option<&CloudSyncScan>,
    local: Option<&LocalSyncScan>,
) {
    let can_delete = !actions
        .iter()
        .any(|action| matches!(action, SyncAction::Conflict { .. }));
    for action in actions {
        match action {
            SyncAction::Conflict { rel_path, reason } => {
                report.record_failure(rel_path, &anyhow::anyhow!("{reason}"), (*reason).into());
            }
            SyncAction::Delete {
                rel_path, target, ..
            } => {
                let file = match target {
                    SyncTarget::Cloud => {
                        cloud.is_some_and(|scan| scan.files.contains_key(rel_path))
                    }
                    SyncTarget::Local => local.is_some_and(|scan| scan.contains_file(rel_path)),
                };
                if can_delete && file {
                    report.record_deleted(rel_path);
                }
            }
            _ => report.record_file_action(action, cloud),
        }
    }
}

fn count_skips(actions: &[SyncAction]) -> usize {
    actions
        .iter()
        .filter(|a| matches!(a, SyncAction::Skip { .. }))
        .count()
}

fn summarize_actions(actions: &[SyncAction]) -> SyncSummary {
    let mut summary = SyncSummary {
        skipped: count_skips(actions),
        ..SyncSummary::default()
    };
    for action in actions
        .iter()
        .filter(|a| !matches!(a, SyncAction::Skip { .. }))
    {
        merge_summary(&mut summary, summary_for_success(action));
    }
    summary
}

fn summary_for_success(action: &SyncAction) -> SyncSummary {
    match action {
        SyncAction::Upload { size, .. } | SyncAction::Download { size, .. } => SyncSummary {
            transferred: 1,
            bytes: *size,
            ..SyncSummary::default()
        },
        SyncAction::CreateDir { .. } => SyncSummary {
            created_dirs: 1,
            ..SyncSummary::default()
        },
        SyncAction::Delete { .. } => SyncSummary {
            deleted: 1,
            ..SyncSummary::default()
        },
        SyncAction::Skip { .. } => SyncSummary {
            skipped: 1,
            ..SyncSummary::default()
        },
        SyncAction::Conflict { .. } => SyncSummary {
            failed: 1,
            ..SyncSummary::default()
        },
    }
}

fn merge_summary(total: &mut SyncSummary, next: SyncSummary) {
    total.transferred += next.transferred;
    total.skipped += next.skipped;
    total.deleted += next.deleted;
    total.created_dirs += next.created_dirs;
    total.failed += next.failed;
    total.bytes += next.bytes;
}

pub(super) fn make_action_progress(
    progress: Option<&ProgressFactory<'_>>,
    action: &SyncAction,
) -> Option<Progress> {
    let length = match action {
        SyncAction::Upload { size, .. } | SyncAction::Download { size, .. } => *size,
        _ => 0,
    };
    progress?(action_rel_path(action), length)
}

fn action_rel_path(action: &SyncAction) -> &str {
    match action {
        SyncAction::Upload { rel_path, .. }
        | SyncAction::Download { rel_path, .. }
        | SyncAction::CreateDir { rel_path, .. }
        | SyncAction::Delete { rel_path, .. }
        | SyncAction::Conflict { rel_path, .. }
        | SyncAction::Skip { rel_path } => rel_path,
    }
}

async fn execute_one_action(
    config: &crate::config::Config,
    action: &SyncAction,
    pb: Option<Progress>,
    remote: &SyncCloud<'_>,
    cloud: Option<&CloudSyncScan>,
    local: Option<&LocalSyncScan>,
    file_changed: &mut bool,
) -> Result<()> {
    match action {
        SyncAction::Upload {
            local_abs,
            rel_path,
            remote_abs,
            replace_directory_id,
            ..
        } => {
            // Validate the source before creating any missing cloud parents.
            super::upload_service::checked_upload_size(local_abs)?;
            let remote_dir = remote_parent(remote_abs);
            let file_name = local_abs
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| anyhow::anyhow!("无法读取本地文件名: {}", local_abs.display()))?;
            let parent_id = remote.ensure_directory(&remote_dir).await?;
            let listing = remote.listing().await?;
            use crate::services::upload_service::transfer::{
                SyncUploadPlan, upload_sync_to_parent,
            };
            upload_sync_to_parent(
                config,
                local_abs,
                SyncUploadPlan {
                    host: listing.host(),
                    scan: cloud,
                    rel_path,
                    parent_id: &parent_id,
                    remote_parent: &remote_dir,
                    file_name,
                    replace_directory_id: replace_directory_id.as_deref(),
                    hash_missing: None,
                    file_changed,
                },
                pb,
            )
            .await?;
        }
        SyncAction::Download {
            rel_path,
            remote_abs,
            local_abs,
            cloud_mtime,
            ..
        } => {
            safe_local(local_abs)?;
            ensure!(
                !local_abs.is_dir(),
                "本地同名目录阻止下载: {}",
                local_abs.display()
            );
            // 下载是读操作，断连就地重试；每次重试从头写本地文件（download 内部重建）。
            let local_path = local_abs.to_string_lossy().to_string();
            let scanned = cloud
                .map(|scan| {
                    let file = scan.files.get(rel_path).context("扫描缺少云端文件")?;
                    let id = file.id.as_deref().context("云端文件缺少有效的扫描 ID")?;
                    Ok::<_, anyhow::Error>((scan.host.as_str(), id, file.size))
                })
                .transpose()?;
            crate::utils::retry::with_retries(RETRIES, BACKOFF_SECS, || {
                let local_path = local_path.clone();
                let attempt_pb = pb.clone();
                async move {
                    if let Some((host, id, size)) = scanned {
                        super::download_service::download_scanned_personal(
                            config, host, id, local_abs, attempt_pb, size,
                        )
                        .await?;
                        Ok(())
                    } else {
                        crate::services::download_service::download(
                            config,
                            remote_abs,
                            &local_path,
                            attempt_pb,
                            false,
                        )
                        .await
                    }
                }
            })
            .await?;
            *file_changed = true;
            if let Some(mtime) = cloud_mtime {
                let file_time = filetime::FileTime::from_unix_time(*mtime, 0);
                filetime::set_file_mtime(local_abs, file_time)?;
            }
        }
        SyncAction::Delete {
            rel_path,
            target,
            target_abs,
        } => match target {
            SyncTarget::Local => {
                let path = Path::new(target_abs);
                let metadata = local
                    .context("删除本地条目需要扫描记录")?
                    .confirm(rel_path, path)?;
                if metadata.is_file() {
                    tokio::fs::remove_file(path).await?;
                    *file_changed = true;
                } else {
                    tokio::fs::remove_dir(path).await?;
                }
            }
            SyncTarget::Cloud => {
                remote.delete_extra(rel_path).await?;
                *file_changed = cloud.is_some_and(|scan| scan.files.contains_key(rel_path));
            }
        },
        SyncAction::CreateDir {
            target, target_abs, ..
        } => match target {
            SyncTarget::Cloud => {
                remote.ensure_directory(target_abs).await?;
            }
            SyncTarget::Local => {
                let path = Path::new(target_abs);
                safe_local(path)?;
                tokio::fs::create_dir_all(path).await?;
            }
        },
        SyncAction::Conflict { reason, .. } => anyhow::bail!("{reason}"),
        SyncAction::Skip { .. } => {}
    }

    Ok(())
}

/// 与 sync 的列表重试保持一致（见 sync_service::SYNC_RETRIES）。
const RETRIES: u32 = crate::services::sync_service::SYNC_RETRIES;
const BACKOFF_SECS: u64 = crate::services::sync_service::SYNC_BACKOFF_SECS;

fn remote_parent(remote_abs: &str) -> String {
    let trimmed = remote_abs.trim_end_matches('/');
    match trimmed.rsplit_once('/') {
        Some(("", _)) | None => "/".to_string(),
        Some((parent, _)) => parent.to_string(),
    }
}
