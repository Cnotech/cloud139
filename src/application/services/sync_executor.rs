use crate::application::services::sync_service::format_action_line;
use crate::domain::{SyncAction, SyncSummary, SyncTarget};
use anyhow::Result;
use futures_util::StreamExt;
use futures_util::stream::FuturesUnordered;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{Mutex, Semaphore};

#[derive(Debug, Clone, Copy)]
pub struct SyncExecuteOptions {
    pub dry_run: bool,
    pub jobs: usize,
    pub progress: bool,
    pub print_actions: bool,
}

pub async fn execute_sync_actions(
    config: &crate::config::Config,
    actions: Vec<SyncAction>,
    options: SyncExecuteOptions,
) -> Result<SyncSummary> {
    if options.dry_run {
        return Ok(summarize_actions(&actions));
    }

    let dir_cache: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
    let semaphore = Arc::new(Semaphore::new(options.jobs.max(1)));
    let multiprogress = Arc::new(MultiProgress::new());
    let skip_count = count_skips(&actions);
    let mut futures = FuturesUnordered::new();

    for action in actions.iter() {
        if matches!(action, SyncAction::Skip { .. }) {
            continue;
        }

        let permit = semaphore.clone().acquire_owned().await?;
        let config = config.clone();
        let pb = make_action_progress(&multiprogress, action, options.progress);
        let action = action.clone();
        let dir_cache = dir_cache.clone();

        futures.push(tokio::spawn(async move {
            if let SyncAction::Upload { remote_abs, .. } = &action
                && let Err(e) = ensure_upload_parent_dir(&config, remote_abs, &dir_cache).await
            {
                drop(permit);
                return (action, Err(e));
            }

            let result = execute_one_action(&config, &action, pb).await;
            drop(permit);
            (action, result)
        }));
    }

    let mut summary = SyncSummary {
        skipped: skip_count,
        ..SyncSummary::default()
    };

    while let Some(result) = futures.next().await {
        match result {
            Ok((action, Ok(()))) => {
                if options.print_actions {
                    eprintln!("{}", format_action_line(&action, false));
                }
                merge_summary(&mut summary, summary_for_success(&action));
            }
            Ok((action, Err(err))) => {
                eprintln!("同步失败: {}: {}", action_rel_path(&action), err);
                summary.failed += 1;
            }
            Err(e) if e.is_panic() => {
                eprintln!("同步任务异常终止");
                summary.failed += 1;
            }
            Err(_) => {
                summary.failed += 1;
            }
        }
    }

    Ok(summary)
}

fn count_skips(actions: &[SyncAction]) -> usize {
    actions
        .iter()
        .filter(|a| matches!(a, SyncAction::Skip { .. }))
        .count()
}

async fn ensure_upload_parent_dir(
    config: &crate::config::Config,
    remote_abs: &str,
    cache: &Arc<Mutex<HashSet<String>>>,
) -> Result<()> {
    let parent = remote_parent(remote_abs);
    if parent == "/" {
        return Ok(());
    }

    let mut dirs_to_ensure = Vec::new();
    let mut current = String::new();
    for part in parent.trim_matches('/').split('/') {
        if current.is_empty() {
            current = format!("/{}", part);
        } else {
            current = format!("{}/{}", current, part);
        }
        dirs_to_ensure.push(current.clone());
    }

    // NOTE on cache semantics: when ensure_personal_cloud_dir fails for a
    // sub-directory, successfully created parent directories remain in the
    // cache. This is intentional: the parent dirs were verified as existing
    // (either just created or previously cached), so retrying them is
    // unnecessary. Only the failed dir itself is removed from the cache so
    // that a future retry can attempt it again.
    for dir in dirs_to_ensure {
        let needs_create = {
            let mut cache_guard = cache.lock().await;
            if cache_guard.contains(&dir) {
                false
            } else {
                cache_guard.insert(dir.clone());
                true
            }
        };

        if needs_create && let Err(e) = ensure_personal_cloud_dir(config, &dir).await {
            let mut cache_guard = cache.lock().await;
            cache_guard.remove(&dir);
            eprintln!("创建云端目录失败: {}", e);
            return Err(e);
        }
    }

    Ok(())
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
        SyncAction::Delete { .. } => SyncSummary {
            deleted: 1,
            ..SyncSummary::default()
        },
        SyncAction::Skip { .. } => SyncSummary {
            skipped: 1,
            ..SyncSummary::default()
        },
    }
}

fn merge_summary(total: &mut SyncSummary, next: SyncSummary) {
    total.transferred += next.transferred;
    total.skipped += next.skipped;
    total.deleted += next.deleted;
    total.failed += next.failed;
    total.bytes += next.bytes;
}

fn make_action_progress(
    multiprogress: &MultiProgress,
    action: &SyncAction,
    enabled: bool,
) -> Option<ProgressBar> {
    if !enabled {
        return None;
    }

    let length = match action {
        SyncAction::Upload { size, .. } | SyncAction::Download { size, .. } => *size,
        _ => 0,
    };
    let pb = multiprogress.add(ProgressBar::new(length));
    let style = ProgressStyle::with_template(
        "{msg} {bar:24.cyan/blue} {bytes}/{total_bytes} {bytes_per_sec} {eta}",
    )
    .unwrap_or_else(|_| ProgressStyle::default_bar());
    pb.set_style(style);
    pb.set_message(action_rel_path(action));
    Some(pb)
}

fn action_rel_path(action: &SyncAction) -> String {
    match action {
        SyncAction::Upload { rel_path, .. }
        | SyncAction::Download { rel_path, .. }
        | SyncAction::Delete { rel_path, .. }
        | SyncAction::Skip { rel_path } => rel_path.clone(),
    }
}

async fn execute_one_action(
    config: &crate::config::Config,
    action: &SyncAction,
    pb: Option<ProgressBar>,
) -> Result<()> {
    match action {
        SyncAction::Upload {
            local_abs,
            remote_abs,
            ..
        } => {
            let remote_dir = remote_parent(remote_abs);
            let file_name = local_abs
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| anyhow::anyhow!("无法读取本地文件名: {}", local_abs.display()))?;
            crate::commands::upload::personal::upload(
                config,
                local_abs,
                &remote_dir,
                file_name,
                std::fs::metadata(local_abs)?.len() as i64,
                true,
                pb,
            )
            .await?;
        }
        SyncAction::Download {
            remote_abs,
            local_abs,
            ..
        } => {
            if let Some(parent) = local_abs.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            crate::application::services::download(
                config,
                remote_abs,
                &local_abs.to_string_lossy(),
                pb,
            )
            .await?;
        }
        SyncAction::Delete {
            target, target_abs, ..
        } => match target {
            SyncTarget::Local => {
                let path = Path::new(target_abs);
                if path.exists() {
                    tokio::fs::remove_file(path).await?;
                }
            }
            SyncTarget::Cloud => {
                crate::application::services::delete(config, target_abs, false).await?;
            }
        },
        SyncAction::Skip { .. } => {}
    }

    Ok(())
}

fn remote_parent(remote_abs: &str) -> String {
    let trimmed = remote_abs.trim_end_matches('/');
    match trimmed.rsplit_once('/') {
        Some(("", _)) | None => "/".to_string(),
        Some((parent, _)) => parent.to_string(),
    }
}

async fn ensure_personal_cloud_dir(config: &crate::config::Config, path: &str) -> Result<()> {
    let (parent, name) = crate::commands::mkdir::parse_path(path)?;
    let mut config = config.clone();
    let host = crate::client::api::get_personal_cloud_host(&mut config).await?;

    let parent_file_id = if parent == "/" {
        "/".to_string()
    } else {
        match crate::client::api::get_file_id_by_path(&config, &parent).await {
            Ok(id) => id,
            Err(e) => {
                return Err(anyhow::anyhow!("获取父目录失败: {}", e));
            }
        }
    };

    let body = serde_json::json!({
        "parentFileId": parent_file_id,
        "name": name,
        "description": "",
        "type": "folder",
        "fileRenameMode": "force_rename"
    });

    let resp: crate::models::PersonalUploadResp = crate::client::api::personal_api_request(
        &config,
        &format!("{}/file/create", host),
        body,
        crate::client::StorageType::PersonalNew,
    )
    .await?;

    if resp.base.success {
        Ok(())
    } else {
        let is_conflict = resp.base.code.as_deref() == Some("409")
            || resp
                .base
                .message
                .as_deref()
                .map(|m| m.contains("已存在"))
                .unwrap_or(false);
        if is_conflict {
            Ok(())
        } else {
            let msg = resp
                .base
                .message
                .unwrap_or_else(|| "创建云端目录失败".to_string());
            Err(crate::client::ClientError::Api(msg).into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ChangeKind;
    use std::collections::HashSet;

    #[tokio::test]
    async fn test_dir_cache_prevents_duplicate_entries() {
        let cache: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));

        cache.lock().await.insert("/a".to_string());

        let needs_create = {
            let mut guard = cache.lock().await;
            if guard.contains("/a") {
                false
            } else {
                guard.insert("/a".to_string());
                true
            }
        };
        assert!(!needs_create, "already-cached dir should not need creation");

        let needs_create_new = {
            let mut guard = cache.lock().await;
            if guard.contains("/b") {
                false
            } else {
                guard.insert("/b".to_string());
                true
            }
        };
        assert!(needs_create_new, "new dir should need creation");
        assert!(cache.lock().await.contains("/b"));
    }

    #[tokio::test]
    async fn test_dir_cache_remove_on_failure() {
        let cache: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));

        cache.lock().await.insert("/failed_dir".to_string());
        let mut guard = cache.lock().await;
        guard.remove("/failed_dir");
        drop(guard);

        let needs_create = {
            let mut guard = cache.lock().await;
            if guard.contains("/failed_dir") {
                false
            } else {
                guard.insert("/failed_dir".to_string());
                true
            }
        };
        assert!(needs_create, "removed dir should need creation again");
    }

    #[test]
    fn test_remote_parent_extracts_parent_path() {
        assert_eq!(remote_parent("/a/b/c.txt"), "/a/b");
        assert_eq!(remote_parent("/a/b.txt"), "/a");
        assert_eq!(remote_parent("/a.txt"), "/");
        assert_eq!(remote_parent("/"), "/");
    }

    #[test]
    fn test_count_skips() {
        let actions = vec![
            SyncAction::Skip {
                rel_path: "a.txt".to_string(),
            },
            SyncAction::Skip {
                rel_path: "b.txt".to_string(),
            },
            SyncAction::Upload {
                rel_path: "c.txt".to_string(),
                local_abs: std::path::PathBuf::from("local/c.txt"),
                remote_abs: "/remote/c.txt".to_string(),
                size: 10,
                change: ChangeKind::New,
            },
        ];
        assert_eq!(count_skips(&actions), 2);
    }
}
