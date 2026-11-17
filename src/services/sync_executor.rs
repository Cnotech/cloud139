use crate::debug;
use crate::domain::{ChangeKind, SyncAction, SyncEntryKind, SyncSummary, SyncTarget};
use crate::services::sync_service::{SyncDiffStats, format_action_line};
use crate::utils::logger::{mp_error, mp_step};
use anyhow::Result;
use futures_util::StreamExt;
use futures_util::stream::FuturesUnordered;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
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
            // 扫描已给出父目录 id 时，父目录必然存在，无需再逐级 ensure。
            if let SyncAction::Upload {
                remote_abs,
                parent_file_id,
                ..
            } = &action
                && parent_file_id.is_none()
                && let Err(e) = ensure_upload_parent_dir(&config, remote_abs, &dir_cache, &mp).await
            {
                drop(permit);
                return (action, Err(e));
            }

            let result =
                execute_one_action(&config, &action, pb, Some(&dir_cache), Some(&mp)).await;
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
    ensure_parent_path(config, &remote_parent(remote_abs), cache, mp).await
}

/// 逐级确保云端目录 parent 及其祖先存在（带幂等缓存）。
async fn ensure_parent_path(
    config: &crate::config::Config,
    parent: &str,
    cache: &Arc<Mutex<HashSet<String>>>,
    mp: &MultiProgress,
) -> Result<()> {
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
        ensure_personal_cloud_dir_cached(config, &dir, cache, Some(mp), None).await?;
    }

    Ok(())
}

async fn ensure_personal_cloud_dir_cached(
    config: &crate::config::Config,
    path: &str,
    cache: &Arc<Mutex<HashSet<String>>>,
    mp: Option<&MultiProgress>,
    parent_file_id: Option<&str>,
) -> Result<()> {
    let needs_create = {
        let mut cache_guard = cache.lock().await;
        if cache_guard.contains(path) {
            false
        } else {
            cache_guard.insert(path.to_string());
            true
        }
    };

    if !needs_create {
        return Ok(());
    }

    if let Err(e) = ensure_personal_cloud_dir(config, path, parent_file_id).await {
        let mut cache_guard = cache.lock().await;
        cache_guard.remove(path);
        if let Some(mp) = mp {
            mp_error(&format!("创建云端目录失败: {}", e), mp);
        }
        return Err(e);
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
    dir_cache: Option<&Arc<Mutex<HashSet<String>>>>,
    mp: Option<&MultiProgress>,
) -> Result<()> {
    match action {
        SyncAction::Upload {
            local_abs,
            remote_abs,
            change,
            parent_file_id,
            file_id,
            ..
        } => {
            let remote_dir = remote_parent(remote_abs);
            let file_name = local_abs
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| anyhow::anyhow!("无法读取本地文件名: {}", local_abs.display()))?;
            if should_pre_delete(*change) {
                // 扫描已给出旧文件 id 时直接按 id 删，省掉一次路径解析。
                match file_id.as_deref().filter(|id| !id.is_empty()) {
                    Some(id) => {
                        crate::services::delete_service::delete_personal_by_id(config, id)
                            .await
                            .ok();
                    }
                    None => {
                        crate::services::delete(config, remote_abs, true).await.ok();
                    }
                }
            }
            crate::services::upload::personal::upload(
                config,
                local_abs,
                &remote_dir,
                file_name,
                std::fs::metadata(local_abs)?.len() as i64,
                true,
                pb,
                parent_file_id.as_deref(),
            )
            .await?;
        }
        SyncAction::Download {
            remote_abs,
            local_abs,
            cloud_mtime,
            file_id,
            parent_file_id,
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
            crate::utils::retry::with_retries(RETRIES, BACKOFF_SECS, || {
                let local_path = local_path.clone();
                let attempt_pb = pb.clone();
                async move {
                    crate::services::download_service::download_personal_for_sync(
                        config,
                        remote_abs,
                        file_id.as_deref(),
                        parent_file_id.as_deref(),
                        &local_path,
                        attempt_pb,
                    )
                    .await
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
                crate::services::delete(config, target_abs, false).await?;
            }
        },
        SyncAction::CreateDir {
            target,
            target_abs,
            parent_file_id,
            ..
        } => match target {
            SyncTarget::Cloud => {
                if let Some(cache) = dir_cache {
                    ensure_personal_cloud_dir_cached(
                        config,
                        target_abs,
                        cache,
                        mp,
                        parent_file_id.as_deref(),
                    )
                    .await?;
                } else {
                    ensure_personal_cloud_dir(config, target_abs, parent_file_id.as_deref())
                        .await?;
                }
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

pub(crate) async fn ensure_personal_cloud_dir(
    config: &crate::config::Config,
    path: &str,
    parent_file_id: Option<&str>,
) -> Result<()> {
    let (parent, name) = crate::services::mkdir_service::parse_path(path)?;
    let config = config.clone();

    // 扫描已给出父目录 id 时直接复用，省掉一次路径解析。
    let parent_file_id = match parent_file_id.filter(|id| !id.is_empty()) {
        Some(id) => id.to_string(),
        None if parent == "/" => "/".to_string(),
        None => {
            match crate::utils::retry::with_retries(RETRIES, BACKOFF_SECS, || async {
                crate::client::api::get_file_id_by_path(&config, &parent)
                    .await
                    .map_err(anyhow::Error::from)
            })
            .await
            {
                Ok(id) => id,
                Err(e) => {
                    return Err(anyhow::anyhow!("获取父目录失败: {}", e));
                }
            }
        }
    };

    // 查找同名条目并判断类型
    let existing = crate::utils::retry::with_retries(RETRIES, BACKOFF_SECS, || async {
        crate::client::api::list_personal_files(&config, &parent_file_id)
            .await
            .map_err(anyhow::Error::from)
    })
    .await?
    .into_iter()
    .find(|item| item.name.as_deref() == Some(&name));

    if let Some(item) = existing {
        if crate::services::sync_service::should_treat_personal_item_as_folder(&item) {
            // 同名条目已经是目录，无需操作
            return Ok(());
        }
        // 同名条目是文件，先删除再创建目录
        let full_path = if parent == "/" {
            format!("/{}", name)
        } else {
            format!("{}/{}", parent.trim_end_matches('/'), name)
        };
        crate::services::delete(&config, &full_path, false).await?;
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
        crate::domain::StorageType::PersonalNew,
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

/// 一个失败对象：JSON 的 failures 元素。
#[derive(Debug, Clone, Serialize)]
pub struct SyncFailure {
    pub path: String,
    pub error: String,
    pub retries: u32,
}

/// --verify --json 的七字段汇总（只计文件，不含目录）。
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncReport {
    pub objects: usize,
    pub uploaded: usize,
    pub skipped: usize,
    pub deleted: usize,
    pub hash_missing: usize,
    pub seconds: f64,
    pub failures: Vec<SyncFailure>,
}

#[derive(Debug, Clone, Copy)]
pub struct SyncVerifyOptions {
    pub dry_run: bool,
    pub delete: bool,
    pub progress: bool,
    pub print_actions: bool,
}

/// --verify (LocalToCloud) 的执行器：串行、删除闸门、安全替换。
pub async fn execute_verified_sync(
    config: &crate::config::Config,
    actions: Vec<SyncAction>,
    stats: SyncDiffStats,
    options: SyncVerifyOptions,
) -> Result<SyncReport> {
    let started = Instant::now();
    let mut report = SyncReport {
        objects: stats.objects,
        skipped: stats.skipped_files,
        hash_missing: stats.hash_missing,
        ..SyncReport::default()
    };
    let delete_actions: Vec<SyncAction> = actions
        .iter()
        .filter(|action| matches!(action, SyncAction::Delete { .. }))
        .cloned()
        .collect();

    if options.dry_run {
        report.uploaded = actions
            .iter()
            .filter(|action| matches!(action, SyncAction::Upload { .. }))
            .count();
        report.deleted = if options.delete {
            stats.planned_deleted_files
        } else {
            0
        };
        report.seconds = started.elapsed().as_secs_f64();
        return Ok(report);
    }

    let multiprogress = MultiProgress::new();
    let dir_cache: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));

    for action in &actions {
        match action {
            SyncAction::Skip { .. } | SyncAction::Delete { .. } => {}
            SyncAction::CreateDir {
                target,
                target_abs,
                parent_file_id,
                rel_path,
                ..
            } if *target == SyncTarget::Cloud => {
                let result = ensure_personal_cloud_dir_cached(
                    config,
                    target_abs,
                    &dir_cache,
                    Some(&multiprogress),
                    parent_file_id.as_deref(),
                )
                .await;
                match result {
                    Ok(()) => {
                        if options.print_actions {
                            mp_step(&format_action_line(action, false), &multiprogress);
                        }
                    }
                    Err(err) => push_failure(&mut report, &multiprogress, rel_path, err),
                }
            }
            SyncAction::Upload {
                change, rel_path, ..
            } => {
                let result = if *change == ChangeKind::New {
                    verified_upload_new(config, &dir_cache, &multiprogress, action, &options).await
                } else {
                    verified_replace(config, &dir_cache, &multiprogress, action, &options).await
                };
                match result {
                    Ok(()) => {
                        report.uploaded += 1;
                        if options.print_actions {
                            mp_step(&format_action_line(action, false), &multiprogress);
                        }
                    }
                    Err(err) => push_failure(&mut report, &multiprogress, rel_path, err),
                }
            }
            _ => {}
        }
    }

    // 删除闸门：任一对象失败 -> 一个都不删；全部成功才按冻结集合删除。
    if options.delete {
        if report.failures.is_empty() {
            for action in &delete_actions {
                match execute_delete_action(config, action).await {
                    Ok(()) => {
                        if matches!(
                            action,
                            SyncAction::Delete {
                                kind: SyncEntryKind::File,
                                ..
                            }
                        ) {
                            report.deleted += 1;
                        }
                    }
                    Err(err) => {
                        let path = match action {
                            SyncAction::Delete { rel_path, .. } => rel_path.as_str(),
                            _ => "",
                        };
                        push_failure(&mut report, &multiprogress, path, err);
                    }
                }
            }
        } else {
            mp_error(
                &format!(
                    "有 {} 个对象处理失败，本轮不执行删除（计划删除 {} 个）",
                    report.failures.len(),
                    delete_actions
                        .iter()
                        .filter(|a| matches!(
                            a,
                            SyncAction::Delete {
                                kind: SyncEntryKind::File,
                                ..
                            }
                        ))
                        .count()
                ),
                &multiprogress,
            );
        }
    }

    report.seconds = started.elapsed().as_secs_f64();
    Ok(report)
}

fn push_failure(report: &mut SyncReport, mp: &MultiProgress, path: &str, err: anyhow::Error) {
    let message = format!("{err:#}");
    mp_error(&format!("同步失败: {}: {}", path, message), mp);
    report.failures.push(SyncFailure {
        path: path.to_string(),
        error: message,
        retries: 0,
    });
}

/// 远端缺失时的新上传：复用扫描父目录 id，缺 id 才逐级 ensure 后解析。
async fn verified_upload_new(
    config: &crate::config::Config,
    cache: &Arc<Mutex<HashSet<String>>>,
    mp: &MultiProgress,
    action: &SyncAction,
    options: &SyncVerifyOptions,
) -> Result<()> {
    let (local_abs, remote_abs, parent_file_id) = match action {
        SyncAction::Upload {
            local_abs,
            remote_abs,
            parent_file_id,
            ..
        } => (local_abs, remote_abs, parent_file_id),
        _ => unreachable!("verified_upload_new only for Upload"),
    };
    let remote_dir = remote_parent(remote_abs);
    let parent_id =
        resolve_parent_id(config, &remote_dir, parent_file_id.as_deref(), cache, mp).await?;
    let file_name = remote_abs.rsplit('/').next().unwrap_or(remote_abs);
    let size = std::fs::metadata(local_abs)?.len() as i64;
    let pb = make_action_progress(mp, action, options.progress);
    crate::services::upload::personal::upload(
        config,
        local_abs,
        &remote_dir,
        file_name,
        size,
        true,
        pb,
        Some(&parent_id),
    )
    .await?;

    // 上传后当场复核：size + contentHash（内容寻址对象缺哈希时按文件名保证）。
    // 新对象传坏当晚就发现，不拖到下一轮。不一致记 failure。
    let verified = verify_uploaded_file(
        config,
        &parent_id,
        file_name,
        file_name,
        size as u64,
        local_abs,
    )
    .await?;
    if let Err(reason) = verified.outcome {
        return Err(anyhow::anyhow!("上传后校验失败: {}", reason));
    }
    Ok(())
}

/// 安全替换：新内容传到同目录临时名 -> 校验临时文件 -> 删旧 -> 改名到目标名。
async fn verified_replace(
    config: &crate::config::Config,
    cache: &Arc<Mutex<HashSet<String>>>,
    mp: &MultiProgress,
    action: &SyncAction,
    options: &SyncVerifyOptions,
) -> Result<()> {
    let (local_abs, remote_abs, parent_file_id, old_file_id) = match action {
        SyncAction::Upload {
            local_abs,
            remote_abs,
            parent_file_id,
            file_id,
            ..
        } => (local_abs, remote_abs, parent_file_id, file_id),
        _ => unreachable!("verified_replace only for Upload"),
    };
    let remote_dir = remote_parent(remote_abs);
    let target_name = remote_abs
        .rsplit('/')
        .next()
        .unwrap_or(remote_abs)
        .to_string();
    let parent_id =
        resolve_parent_id(config, &remote_dir, parent_file_id.as_deref(), cache, mp).await?;
    let size = std::fs::metadata(local_abs)?.len() as i64;
    let temp_name = format!(
        "{}.cloud139-tmp-{}",
        target_name,
        crate::utils::crypto::generate_random_string(8)
    );
    let temp_path = join_remote_path(&remote_dir, &temp_name);

    // 1) 新内容先传到临时名（旧文件一直没动）。
    let pb = make_action_progress(mp, action, options.progress);
    crate::services::upload::personal::upload(
        config,
        local_abs,
        &remote_dir,
        &temp_name,
        size,
        true,
        pb,
        Some(&parent_id),
    )
    .await?;

    // 2) 重新列父目录校验临时文件（size + contentHash / 文件名即哈希）。
    let verify = verify_uploaded_file(
        config,
        &parent_id,
        &temp_name,
        &target_name,
        size as u64,
        local_abs,
    )
    .await;
    let verified = match verify {
        Ok(v) => v,
        Err(err) => {
            // 校验请求本身失败：删临时名，旧文件保持不动。
            let _ = crate::services::delete(config, &temp_path, true).await;
            return Err(err);
        }
    };
    if let Err(reason) = verified.outcome {
        match verified.file_id.as_deref() {
            Some(id) => {
                let _ = crate::services::delete_service::delete_personal_by_id(config, id).await;
            }
            None => {
                let _ = crate::services::delete(config, &temp_path, true).await;
            }
        }
        return Err(anyhow::anyhow!("校验临时文件失败: {}", reason));
    }

    // 3) 校验通过才删旧文件。
    match old_file_id.as_deref().filter(|id| !id.is_empty()) {
        Some(id) => {
            crate::services::delete_service::delete_personal_by_id(config, id).await?;
        }
        None => {
            crate::services::delete(config, remote_abs, true).await?;
        }
    }

    // 4) 临时名改名到目标名。
    let temp_id = verified
        .file_id
        .ok_or_else(|| anyhow::anyhow!("校验后缺少临时文件 id"))?;
    rename_personal_file_by_id(config, &temp_id, &target_name).await?;
    Ok(())
}

struct TempVerify {
    file_id: Option<String>,
    outcome: std::result::Result<(), String>,
}

async fn verify_uploaded_file(
    config: &crate::config::Config,
    parent_id: &str,
    temp_name: &str,
    target_name: &str,
    expected_size: u64,
    local_abs: &Path,
) -> Result<TempVerify> {
    let items = crate::utils::retry::with_retries(RETRIES, BACKOFF_SECS, || async {
        crate::client::api::list_personal_files(config, parent_id)
            .await
            .map_err(anyhow::Error::from)
    })
    .await?;
    let Some(item) = items
        .iter()
        .find(|item| item.name.as_deref() == Some(temp_name))
    else {
        return Ok(TempVerify {
            file_id: None,
            outcome: Err("上传后未在远端找到文件".to_string()),
        });
    };
    let file_id = item.file_id.clone().filter(|id| !id.is_empty());
    let outcome = if item.size.unwrap_or(-1).max(0) as u64 != expected_size {
        Err("远端文件大小与本地不一致".to_string())
    } else if item.content_hash.is_some()
        && crate::services::sync_service::is_sha256_algorithm(
            item.content_hash_algorithm.as_deref(),
        )
    {
        let expected = if crate::services::sync_service::is_content_addressed(target_name) {
            target_name.to_lowercase()
        } else {
            crate::services::sync_service::expected_sha256(target_name, local_abs)
                .unwrap_or_default()
        };
        match item.content_hash.as_deref() {
            Some(hash) if crate::services::sync_service::hash_matches(hash, &expected) => Ok(()),
            _ => Err("远端文件 contentHash 与本地不一致".to_string()),
        }
    } else if crate::services::sync_service::is_content_addressed(target_name) {
        // 内容寻址对象：名字即哈希，缺哈希时按文件名保证。
        Ok(())
    } else {
        // v7 批准的降级：普通文件远端未返回可比 contentHash，size 已一致，
        // 无法再比对内容；按「更新」放行，避免每晚误报失败堵住删除闸门。
        Ok(())
    };
    Ok(TempVerify { file_id, outcome })
}

/// 解析目录 id：扫描给了就复用，否则确保父目录存在后按路径解析。
async fn resolve_parent_id(
    config: &crate::config::Config,
    remote_dir: &str,
    scanned_id: Option<&str>,
    cache: &Arc<Mutex<HashSet<String>>>,
    mp: &MultiProgress,
) -> Result<String> {
    if let Some(id) = scanned_id.filter(|id| !id.is_empty()) {
        return Ok(id.to_string());
    }
    if remote_dir == "/" {
        return Ok("/".to_string());
    }
    ensure_parent_path(config, remote_dir, cache, mp).await?;
    let dir = remote_dir.to_string();
    crate::utils::retry::with_retries(RETRIES, BACKOFF_SECS, || async {
        crate::client::api::get_file_id_by_path(config, &dir)
            .await
            .map_err(anyhow::Error::from)
    })
    .await
}

async fn rename_personal_file_by_id(
    config: &crate::config::Config,
    file_id: &str,
    new_name: &str,
) -> Result<()> {
    let mut config = config.clone();
    let host = crate::client::api::get_personal_cloud_host(&mut config).await?;
    let url = format!("{}/file/update", host);
    let body = serde_json::json!({
        "fileId": file_id,
        "name": new_name,
        "description": ""
    });
    let resp: crate::models::PersonalUploadResp = crate::client::api::personal_api_request(
        &config,
        &url,
        body,
        crate::domain::StorageType::PersonalNew,
    )
    .await?;
    if resp.base.success {
        Ok(())
    } else {
        Err(crate::client::ClientError::Api(
            resp.base
                .message
                .unwrap_or_else(|| "重命名失败".to_string()),
        )
        .into())
    }
}

async fn execute_delete_action(config: &crate::config::Config, action: &SyncAction) -> Result<()> {
    match action {
        SyncAction::Delete {
            target: SyncTarget::Cloud,
            target_abs,
            file_id,
            ..
        } => match file_id.as_deref().filter(|id| !id.is_empty()) {
            Some(id) => {
                crate::services::delete_service::delete_personal_by_id(config, id).await?;
            }
            None => {
                crate::services::delete(config, target_abs, false).await?;
            }
        },
        SyncAction::Delete {
            target: SyncTarget::Local,
            target_abs,
            ..
        } => {
            let path = Path::new(target_abs);
            if path.is_file() {
                tokio::fs::remove_file(path).await?;
            } else if path.is_dir() {
                tokio::fs::remove_dir(path).await?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// 与 sync_service::join_cloud_path 一致：父目录 + 文件名拼成云端绝对路径。
fn join_remote_path(parent: &str, name: &str) -> String {
    if parent == "/" || parent.is_empty() {
        format!("/{}", name.trim_matches('/'))
    } else {
        format!(
            "{}/{}",
            parent.trim_end_matches('/'),
            name.trim_matches('/')
        )
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
                parent_file_id: None,
                file_id: None,
            },
        ];
        assert_eq!(count_skips(&actions), 2);
    }

    #[tokio::test]
    async fn ensure_personal_cloud_dir_deletes_conflicting_file_before_create() {
        let server = MockServer::start();
        let config = Config {
            authorization: "test-auth".to_string(),
            account: "13800138000".to_string(),
            storage_type: "personal".to_string(),
            personal_cloud_host: Some(server.url("")),
            ..Default::default()
        };

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

        ensure_personal_cloud_dir(&config, "/conflict", None)
            .await
            .expect("cloud dir creation should replace same-name file");

        // 路径解析与「检查是否已存在」现在共用同一个翻页 list，所以调用两次。
        list_for_existing.assert_calls(2);
        delete_file.assert_calls(1);
        create_dir.assert_calls(1);
    }

    #[tokio::test]
    async fn concurrent_create_dir_and_upload_parent_ensure_only_create_once() {
        let server = MockServer::start();
        let config = Config {
            authorization: "test-auth".to_string(),
            account: "13800138000".to_string(),
            storage_type: "personal".to_string(),
            personal_cloud_host: Some(server.url("")),
            ..Default::default()
        };

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

        let cache: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
        let mp = MultiProgress::new();
        let create_action = SyncAction::CreateDir {
            rel_path: "docs".to_string(),
            target: SyncTarget::Cloud,
            target_abs: "/remote/docs".to_string(),
            parent_file_id: None,
        };

        let (parent_result, create_result) = tokio::join!(
            ensure_upload_parent_dir(&config, "/remote/docs/file.txt", &cache, &mp),
            execute_one_action(&config, &create_action, None, Some(&cache), Some(&mp)),
        );

        parent_result.expect("upload parent ensure should succeed");
        create_result.expect("create dir action should succeed");

        // 同上：路径解析不再单独发一个 list 请求。
        root_existing_list.assert_calls(2);
        remote_list.assert_calls(1);
        create_dir.assert_calls(1);
    }
}
