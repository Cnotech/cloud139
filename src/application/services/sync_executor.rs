use crate::application::services::sync_service::format_action_line;
use crate::domain::{SyncAction, SyncSummary, SyncTarget};
use crate::utils::logger::{mp_error, mp_step};
use anyhow::Result;
use futures_util::StreamExt;
use futures_util::stream::FuturesUnordered;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::time::Duration;
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{Mutex, Semaphore};

/// 只有已存在的文件（SizeOrTime / Checksum 变更）才需要先删后传。
/// 新文件（New）目标端不存在，pre-delete 是多余的 API 调用。
pub fn should_pre_delete(change: crate::domain::ChangeKind) -> bool {
    !matches!(change, crate::domain::ChangeKind::New)
}

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
        let mp = multiprogress.clone();

        futures.push(tokio::spawn(async move {
            if let SyncAction::Upload { remote_abs, .. } = &action
                && let Err(e) = ensure_upload_parent_dir(&config, remote_abs, &dir_cache, &mp).await
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
                    mp_step(&format_action_line(&action, false), &multiprogress);
                }
                merge_summary(&mut summary, summary_for_success(&action));
            }
            Ok((action, Err(err))) => {
                mp_error(
                    &format!("同步失败: {}: {}", action_rel_path(&action), err),
                    &multiprogress,
                );
                summary.failed += 1;
            }
            Err(e) if e.is_panic() => {
                mp_error("同步任务异常终止", &multiprogress);
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
    mp: &MultiProgress,
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
            mp_error(&format!("创建云端目录失败: {}", e), mp);
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
    // 启动后台定时重绘，确保进度条在 HTTP 请求等待期间也能平滑更新，
    // 避免因 indicatif 节流机制导致进度条看起来从 0% 跳到完成。
    pb.enable_steady_tick(Duration::from_millis(100));
    Some(pb)
}

fn action_rel_path(action: &SyncAction) -> String {
    match action {
        SyncAction::Upload { rel_path, .. }
        | SyncAction::Download { rel_path, .. }
        | SyncAction::CreateDir { rel_path, .. }
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
            change,
            ..
        } => {
            let remote_dir = remote_parent(remote_abs);
            let file_name = local_abs
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| anyhow::anyhow!("无法读取本地文件名: {}", local_abs.display()))?;
            if should_pre_delete(*change) {
                crate::application::services::delete(config, remote_abs, true).await.ok();
            }
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
            cloud_mtime,
            ..
        } => {
            if let Some(parent) = local_abs.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            // If target exists as a directory, remove it first to replace with file
            if local_abs.is_dir() {
                tokio::fs::remove_dir_all(local_abs).await?;
            }
            crate::application::services::download(
                config,
                remote_abs,
                &local_abs.to_string_lossy(),
                pb,
            )
            .await?;
            if let Some(mtime) = cloud_mtime {
                let file_time = filetime::FileTime::from_unix_time(*mtime, 0);
                filetime::set_file_mtime(local_abs, file_time)?;
            }
        }
        SyncAction::Delete {
            target, target_abs, ..
        } => match target {
            SyncTarget::Local => {
                let path = Path::new(target_abs);
                if path.is_file() {
                    tokio::fs::remove_file(path).await?;
                } else if path.is_dir() {
                    tokio::fs::remove_dir(path).await?;
                }
            }
            SyncTarget::Cloud => {
                crate::application::services::delete(config, target_abs, false).await?;
            }
        },
        SyncAction::CreateDir { target, target_abs, .. } => match target {
            SyncTarget::Cloud => {
                ensure_personal_cloud_dir(config, target_abs).await?;
            }
            SyncTarget::Local => {
                let path = Path::new(target_abs);
                // If target exists as a file, remove it first to replace with directory
                if path.is_file() {
                    tokio::fs::remove_file(path).await?;
                }
                tokio::fs::create_dir_all(path).await?;
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

pub(crate) async fn ensure_personal_cloud_dir(config: &crate::config::Config, path: &str) -> Result<()> {
    let (parent, name) = crate::commands::mkdir::parse_path(path)?;
    let config = config.clone();

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

    // 查找同名条目并判断类型
    let existing = crate::client::api::list_personal_files(&config, &parent_file_id)
        .await?
        .into_iter()
        .find(|item| item.name.as_deref() == Some(&name));

    if let Some(item) = existing {
        if crate::application::services::sync_service::should_treat_personal_item_as_folder(&item) {
            // 同名条目已经是目录，无需操作
            return Ok(());
        }
        // 同名条目是文件，先删除再创建目录
        let full_path = if parent == "/" {
            format!("/{}", name)
        } else {
            format!("{}/{}", parent.trim_end_matches('/'), name)
        };
        crate::application::services::delete(&config, &full_path, false).await?;
    }

    // 创建目录
    let mut config_for_create = config.clone();
    let host = crate::client::api::get_personal_cloud_host(&mut config_for_create).await?;

    let body = serde_json::json!({
        "parentFileId": parent_file_id,
        "name": name,
        "description": "",
        "type": "folder"
    });

    let resp: crate::models::PersonalUploadResp = crate::client::api::personal_api_request(
        &config_for_create,
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
    use crate::config::Config;
    use crate::domain::ChangeKind;
    use httpmock::prelude::*;
    use serde_json::json;
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

    #[tokio::test]
    async fn ensure_personal_cloud_dir_deletes_conflicting_file_before_create() {
        let server = MockServer::start();
        let mut config = Config::default();
        config.authorization = "test-auth".to_string();
        config.account = "13800138000".to_string();
        config.storage_type = "personal".to_string();
        config.personal_cloud_host = Some(server.url(""));

        let list_for_existing = server.mock(|when, then| {
            when.method(POST).path("/file/list").json_body(json!({
                "imageThumbnailStyleList": ["Small", "Large"],
                "orderBy": "updated_at",
                "orderDirection": "DESC",
                "pageInfo": {
                    "pageCursor": "",
                    "pageSize": 100
                },
                "parentFileId": "/"
            }));
            then.status(200).json_body(json!({
                "success": true,
                "data": {
                    "items": [
                        {
                            "fileId": "file_1",
                            "name": "conflict",
                            "size": 0,
                            "type": "file"
                        }
                    ],
                    "nextPageCursor": ""
                }
            }));
        });

        let list_for_delete_lookup = server.mock(|when, then| {
            when.method(POST).path("/file/list").json_body(json!({
                "parentFileId": "/",
                "pageInfo": {
                    "pageCursor": "",
                    "pageSize": 100
                },
                "orderBy": "updated_at",
                "orderDirection": "DESC"
            }));
            then.status(200).json_body(json!({
                "success": true,
                "data": {
                    "items": [
                        {
                            "fileId": "file_1",
                            "name": "conflict",
                            "size": 0,
                            "type": "file"
                        }
                    ]
                }
            }));
        });

        let delete_file = server.mock(|when, then| {
            when.method(POST)
                .path("/recyclebin/batchTrash")
                .json_body(json!({
                    "fileIds": ["file_1"]
                }));
            then.status(200).json_body(json!({
                "success": true,
                "message": "文件已移动到回收站"
            }));
        });

        let create_dir = server.mock(|when, then| {
            when.method(POST).path("/file/create").json_body(json!({
                "parentFileId": "/",
                "name": "conflict",
                "description": "",
                "type": "folder"
            }));
            then.status(200).json_body(json!({
                "success": true,
                "data": {
                    "fileId": "dir_1",
                    "fileName": "conflict"
                }
            }));
        });

        ensure_personal_cloud_dir(&config, "/conflict")
            .await
            .expect("cloud dir creation should replace same-name file");

        list_for_existing.assert_calls(1);
        list_for_delete_lookup.assert_calls(1);
        delete_file.assert_calls(1);
        create_dir.assert_calls(1);
    }
}
