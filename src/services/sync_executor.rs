use crate::debug;
use crate::domain::{SyncAction, SyncSummary, SyncTarget};
use crate::services::sync_service::{CloudSyncScan, format_action_line};
use crate::utils::logger::{mp_error, mp_step};
use anyhow::{Context, Result, ensure};
use futures_util::StreamExt;
use futures_util::stream::FuturesUnordered;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, OnceCell, Semaphore};

type DirectoryCache = Mutex<HashMap<String, Arc<OnceCell<String>>>>;

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
    execute_scanned_sync(config, actions, options, None).await
}

pub(crate) async fn execute_scanned_sync(
    config: &crate::config::Config,
    actions: Vec<SyncAction>,
    options: SyncExecuteOptions,
    cloud: Option<Arc<CloudSyncScan>>,
) -> Result<SyncSummary> {
    debug!(
        "execute_sync_actions: {} 个动作, dry_run={}, jobs={}",
        actions
            .iter()
            .filter(|a| !matches!(a, SyncAction::Skip { .. }))
            .count(),
        options.dry_run,
        options.jobs
    );
    if options.dry_run {
        return Ok(summarize_actions(&actions));
    }

    let dir_cache = Arc::new(DirectoryCache::default());
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
        let cloud = cloud.clone();

        futures.push(tokio::spawn(async move {
            let result =
                execute_one_action(&config, &action, pb.clone(), &dir_cache, cloud.as_deref())
                    .await;
            if result.is_err()
                && let Some(pb) = pb
            {
                pb.finish_and_clear();
            }
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
                    if options.progress {
                        mp_step(&format_action_line(&action, false), &multiprogress);
                    } else {
                        crate::step!("{}", format_action_line(&action, false));
                    }
                }
                merge_summary(&mut summary, summary_for_success(&action));
            }
            Ok((action, Err(err))) => {
                let message = format!("同步失败: {}: {err:#}", action_rel_path(&action));
                if options.progress {
                    mp_error(&message, &multiprogress);
                } else {
                    crate::error!("{}", message);
                }
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

async fn ensure_cloud_path(
    config: &crate::config::Config,
    path: &str,
    cache: &DirectoryCache,
) -> Result<String> {
    let path = super::directory_service::normalize(path)?;
    let mut prefix = String::new();
    let mut parent_id = "/".to_owned();
    let mut seen = HashSet::from([parent_id.clone()]);
    for name in path.split('/').filter(|name| !name.is_empty()) {
        prefix.push('/');
        prefix.push_str(name);
        let cell = directory_cell(cache, &prefix).await;
        let id = cell
            .get_or_try_init(|| ensure_personal_child(config, &parent_id, name, &seen))
            .await?;
        ensure!(seen.insert(id.clone()), "目录解析返回重复 ID: {prefix}");
        parent_id.clone_from(id);
    }
    Ok(parent_id)
}

async fn directory_cell(cache: &DirectoryCache, path: &str) -> Arc<OnceCell<String>> {
    cache
        .lock()
        .await
        .entry(path.to_owned())
        .or_default()
        .clone()
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
        | SyncAction::Conflict { rel_path, .. }
        | SyncAction::Skip { rel_path } => rel_path.clone(),
    }
}

async fn execute_one_action(
    config: &crate::config::Config,
    action: &SyncAction,
    pb: Option<ProgressBar>,
    dir_cache: &DirectoryCache,
    cloud: Option<&CloudSyncScan>,
) -> Result<()> {
    match action {
        SyncAction::Upload {
            local_abs,
            remote_abs,
            replace_directory_id,
            ..
        } => {
            // Resolve upload metadata before ensuring parents, which can replace cloud files.
            super::upload_service::checked_upload_size(local_abs)?;
            let remote_dir = remote_parent(remote_abs);
            let file_name = local_abs
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| anyhow::anyhow!("无法读取本地文件名: {}", local_abs.display()))?;
            let parent_id = ensure_cloud_path(config, &remote_dir, dir_cache).await?;
            let mut config = config.clone();
            let host = crate::client::api::get_personal_cloud_host(&mut config).await?;
            use crate::services::upload::personal::{SyncUploadPlan, upload_sync_to_parent};
            upload_sync_to_parent(
                &config,
                local_abs,
                SyncUploadPlan {
                    host: &host,
                    parent_id: &parent_id,
                    file_name,
                    replace_directory_id: replace_directory_id.as_deref(),
                    verification: None,
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
            if let Some(parent) = local_abs.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            // If target exists as a directory, remove it first to replace with file
            if local_abs.is_dir() {
                tokio::fs::remove_dir_all(local_abs).await?;
            }
            // 下载是读操作，断连就地重试；每次重试从头写本地文件（download 内部重建）。
            let local_path = local_abs.to_string_lossy().to_string();
            let scanned = cloud.and_then(|cloud| {
                let file = cloud.files.get(rel_path)?;
                Some((cloud.host.as_str(), file.id.as_deref()?, file.size))
            });
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
                        crate::services::download(config, remote_abs, &local_path, attempt_pb).await
                    }
                }
            })
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
                crate::services::delete_service::delete_recursive(config, target_abs, false, true)
                    .await?;
            }
        },
        SyncAction::CreateDir {
            target, target_abs, ..
        } => match target {
            SyncTarget::Cloud => {
                ensure_cloud_path(config, target_abs, dir_cache).await?;
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

async fn ensure_personal_child(
    config: &crate::config::Config,
    parent_id: &str,
    name: &str,
    ancestors: &HashSet<String>,
) -> Result<String> {
    use crate::client::api;
    let mut config = config.clone();
    let host = api::get_personal_cloud_host(&mut config).await?;
    let list_children = || {
        crate::utils::retry::with_retries(RETRIES, BACKOFF_SECS, || async {
            api::list_personal_files(&config, parent_id)
                .await
                .map_err(Into::into)
        })
    };
    let children = list_children().await?;
    if let Some(item) = unique_child(&children, name)? {
        let id = item
            .file_id
            .as_deref()
            .filter(|id| !id.is_empty() && !ancestors.contains(*id))
            .context("云端子目录条目缺少有效 ID")?;
        if item.is_folder() {
            return Ok(id.to_owned());
        }
        super::delete_service::trash_personal_file_id(&config, &host, id).await?;
    }

    let body = serde_json::json!({
        "parentFileId": parent_id,
        "name": name,
        "description": "",
        "type": "folder"
    });

    let resp: crate::models::PersonalUploadResp = api::personal_api_request(
        &config,
        &format!("{}/file/create", host),
        body,
        crate::domain::StorageType::PersonalNew,
    )
    .await?;

    if resp.base.success
        && let Some(data) = resp.data
        && data.file_name.as_deref() == Some(name)
        && let Some(id) = data
            .file_id
            .filter(|id| !id.is_empty() && !ancestors.contains(id))
    {
        return Ok(id);
    }
    let conflict = resp.base.code.as_deref() == Some("409")
        || resp
            .base
            .message
            .as_deref()
            .is_some_and(|message| message.contains("已存在"));
    ensure!(
        resp.base.success || conflict,
        "{}",
        resp.base.message.as_deref().unwrap_or("创建云端目录失败")
    );
    let children = list_children().await?;
    unique_child(&children, name)?
        .filter(|item| item.is_folder())
        .and_then(|item| item.file_id.as_ref())
        .filter(|id| !id.is_empty() && !ancestors.contains(*id))
        .cloned()
        .context("创建后无法确认云端目录")
}

fn unique_child<'a>(
    children: &'a [crate::models::PersonalFileItem],
    name: &str,
) -> Result<Option<&'a crate::models::PersonalFileItem>> {
    let mut matches = children
        .iter()
        .filter(|item| item.name.as_deref() == Some(name));
    let first = matches.next();
    ensure!(matches.next().is_none(), "云端存在多个同名子条目: {name}");
    Ok(first)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::domain::ChangeKind;
    use httpmock::prelude::*;
    use serde_json::json;
    #[tokio::test]
    async fn test_dir_cache_prevents_duplicate_entries() {
        let cache = DirectoryCache::default();
        let first = directory_cell(&cache, "/a").await;
        assert!(Arc::ptr_eq(&first, &directory_cell(&cache, "/a").await));
        assert!(!Arc::ptr_eq(&first, &directory_cell(&cache, "/b").await));
    }

    #[tokio::test]
    async fn test_dir_cache_allows_retry_on_failure() {
        let cache = DirectoryCache::default();
        let cell = directory_cell(&cache, "/retry").await;
        assert!(
            cell.get_or_try_init(|| async { Err::<String, _>("failed") })
                .await
                .is_err()
        );
        let id = directory_cell(&cache, "/retry").await;
        assert_eq!(
            id.get_or_init(|| async { "created".to_string() }).await,
            "created"
        );
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
                replace_directory_id: None,
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

        let id = ensure_cloud_path(&config, "/conflict", &DirectoryCache::default())
            .await
            .expect("cloud dir creation should replace same-name file");
        assert_eq!(id, "dir_1");
        list_for_existing.assert_calls(1);
        delete_file.assert_calls(1);
        create_dir.assert_calls(1);
    }

    #[tokio::test]
    async fn concurrent_create_dir_and_upload_parent_ensure_only_create_once() {
        let server = MockServer::start();
        let mut config = Config::default();
        config.authorization = "test-auth".to_string();
        config.account = "13800138000".to_string();
        config.storage_type = "personal".to_string();
        config.personal_cloud_host = Some(server.url(""));

        let root_existing_list = server.mock(|when, then| {
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
                            "fileId": "dir_remote",
                            "name": "remote",
                            "size": 0,
                            "type": "folder"
                        }
                    ],
                    "nextPageCursor": ""
                }
            }));
        });

        let remote_list = server.mock(|when, then| {
            when.method(POST).path("/file/list").json_body(json!({
                "imageThumbnailStyleList": ["Small", "Large"],
                "orderBy": "updated_at",
                "orderDirection": "DESC",
                "pageInfo": {
                    "pageCursor": "",
                    "pageSize": 100
                },
                "parentFileId": "dir_remote"
            }));
            then.status(200).json_body(json!({
                "success": true,
                "data": {
                    "items": [],
                    "nextPageCursor": ""
                }
            }));
        });

        let create_dir = server.mock(|when, then| {
            when.method(POST).path("/file/create").json_body(json!({
                "parentFileId": "dir_remote",
                "name": "docs",
                "description": "",
                "type": "folder"
            }));
            then.status(200).json_body(json!({
                "success": true,
                "data": {
                    "fileId": "dir_docs",
                    "fileName": "docs"
                }
            }));
        });

        let cache = DirectoryCache::default();
        let create_action = SyncAction::CreateDir {
            rel_path: "docs".to_string(),
            target: SyncTarget::Cloud,
            target_abs: "/remote/docs".to_string(),
        };

        let (parent_result, create_result) = tokio::join!(
            ensure_cloud_path(&config, "/remote/docs", &cache),
            execute_one_action(&config, &create_action, None, &cache, None),
        );

        assert_eq!(
            parent_result.expect("upload parent ensure should succeed"),
            "dir_docs"
        );
        create_result.expect("create dir action should succeed");

        root_existing_list.assert_calls(1);
        remote_list.assert_calls(1);
        create_dir.assert_calls(1);
    }
}
