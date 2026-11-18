use crate::client::{ClientError, api};
use crate::domain::{FileEntry, SyncEntryKind};
use indicatif::ProgressBar;
use std::collections::HashMap;
use std::path::Path;

use super::directory_service as dirs;

pub(crate) async fn upload_path(
    config: &crate::config::Config,
    local: &str,
    remote: &str,
    force: bool,
    make_progress: impl FnOnce(&str, u64) -> Option<ProgressBar>,
) -> anyhow::Result<()> {
    let local_path = Path::new(local);
    if !local_path.exists() {
        crate::error!("文件不存在: {}", local);
        return Err(ClientError::FileNotFound.into());
    }

    dirs::safe_local(local_path)?;
    if local_path.is_dir() {
        return upload_directory(
            config,
            local_path,
            remote,
            local.ends_with('/') || local.ends_with('\\'),
            force,
        )
        .await;
    }
    if !local_path.is_file() {
        anyhow::bail!("只支持普通文件或目录");
    }
    let file_name = local_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    let file_size = checked_upload_size(local_path)
        .map_err(|error| anyhow::anyhow!("无法上传 {}: {}", local, error))?;

    let remote_dir = if remote == "/" {
        "".to_string()
    } else {
        remote.trim_end_matches('/').to_string()
    };

    crate::debug!(
        "准备上传文件 local={} remote={}/{}",
        local,
        remote_dir,
        file_name
    );
    crate::debug!("已读取本地文件大小 size_bytes={}", file_size);

    let pb = make_progress(file_name, file_size as u64);

    upload(
        config,
        local_path,
        &remote_dir,
        file_name,
        file_size,
        force,
        pb,
    )
    .await?;

    Ok(())
}

/// Re-read metadata before hashing or any destructive sync operation.
pub fn checked_upload_size(path: &Path) -> Result<i64, ClientError> {
    dirs::safe_local(path)?;
    let metadata = std::fs::metadata(path)?;
    if !metadata.is_file() {
        return Err(dirs::invalid("只支持普通文件；目录请使用递归上传"));
    }
    i64::try_from(metadata.len()).map_err(|_| dirs::invalid("文件过大"))
}

pub async fn upload(
    config: &crate::config::Config,
    local_path: &Path,
    remote_path: &str,
    file_name: &str,
    file_size: i64,
    force: bool,
    pb: Option<ProgressBar>,
) -> Result<(), ClientError> {
    config.require_personal()?;
    dirs::validate_name(file_name)?;
    if file_size < 0 {
        return Err(dirs::invalid("无效文件大小"));
    }
    checked_upload_size(local_path)?;
    let remote_path = dirs::normalize(remote_path)?;
    let http_client = api::HttpClientWrapper::new();
    let listing =
        api::PersonalClient::connect(config, &http_client, api::ListingRetry::DIRECTORY).await?;
    let parent = dirs::directory_with_listing(&listing, &remote_path).await?;
    upload_file(&listing, local_path, &parent, file_name, force, pb).await
}

/// Upload `dir` itself, or its contents when the local argument has a trailing separator.
pub async fn upload_directory(
    config: &crate::config::Config,
    local: &Path,
    remote: &str,
    contents: bool,
    force: bool,
) -> anyhow::Result<()> {
    use super::sync_service::{SyncScanOptions, scan_local};
    config.require_personal()?;
    dirs::safe_local(local)?;
    if !local.is_dir() {
        return Err(dirs::invalid("递归上传源必须是目录").into());
    }
    let entries = scan_local(
        local,
        SyncScanOptions {
            recursive: true,
            exclude: vec![],
        },
    )?;
    let remote = dirs::normalize(remote)?;
    let root = if contents {
        remote
    } else {
        let name = local
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or(ClientError::InvalidFilePath)?;
        dirs::validate_name(name)?;
        format!("{}/{}", remote.trim_end_matches('/'), name)
    };
    // Validate the complete local plan before any cloud mutation.
    for entry in &entries {
        for name in entry.rel_path.split('/') {
            dirs::validate_name(name)?;
        }
        dirs::safe_local(&local.join(&entry.rel_path))?;
    }
    upload_personal_directory(config, local, &root, entries, force).await
}

async fn upload_personal_directory(
    config: &crate::config::Config,
    local: &Path,
    root: &str,
    entries: Vec<FileEntry>,
    force: bool,
) -> anyhow::Result<()> {
    if root == "/" && entries.is_empty() {
        return Ok(());
    }
    let http_client = api::HttpClientWrapper::new();
    let listing =
        api::PersonalClient::connect(config, &http_client, api::ListingRetry::DIRECTORY).await?;
    let root_id = dirs::ensure_personal_dir(&listing, root, true).await?;
    let mut directories = HashMap::from([(root.to_owned(), root_id)]);
    let mut uploaded = 0;
    let mut failures = Vec::new();
    for entry in entries {
        let destination = format!("{}/{}", root.trim_end_matches('/'), entry.rel_path);
        if entry.kind == SyncEntryKind::Directory {
            let id = dirs::ensure_personal_dir(&listing, &destination, true)
                .await
                .map_err(|error| {
                    anyhow::anyhow!(
                        "创建目录 {} 失败；此前条目可能已完成: {:#}",
                        entry.rel_path,
                        anyhow::Error::new(error)
                    )
                })?;
            directories.insert(destination, id);
            continue;
        }
        let (parent, name) = destination.rsplit_once('/').unwrap();
        let parent = if parent.is_empty() { "/" } else { parent };
        let parent_id = directories
            .get(parent)
            .ok_or_else(|| dirs::invalid("上传计划缺少父目录"))?;
        let path = local.join(&entry.rel_path);
        let result = upload_file(&listing, &path, parent_id, name, force, None).await;
        match result {
            Ok(()) => uploaded += 1,
            Err(error) => {
                let message = format!(
                    "上传 {} 失败；此前条目可能已完成: {}",
                    entry.rel_path, error
                );
                crate::error!("{}", message);
                failures.push(message);
            }
        }
    }
    if !failures.is_empty() {
        anyhow::bail!(
            "目录上传未完成: 成功 {} 个，失败 {} 个；{}",
            uploaded,
            failures.len(),
            failures.join("；")
        );
    }
    crate::success!("目录上传完成: 成功 {} 个文件", uploaded);
    Ok(())
}

async fn upload_file(
    listing: &api::PersonalClient<'_>,
    path: &Path,
    parent_id: &str,
    name: &str,
    force: bool,
    pb: Option<ProgressBar>,
) -> Result<(), ClientError> {
    let size = checked_upload_size(path)?;
    let items = listing.list_files(parent_id).await?;
    for item in items
        .iter()
        .filter(|item| item.name.as_deref() == Some(name))
    {
        if item.is_folder() {
            return Err(dirs::invalid("云端同名目录阻止上传文件"));
        }
        if !force {
            return Err(ClientError::ForceRequired);
        }
    }
    let saved_name =
        super::upload::personal::upload_to_parent(listing, path, parent_id, name, size, pb.clone())
            .await?;
    crate::utils::logger::pb_success(&format!("上传完成: {saved_name}"), &pb);
    if let Some(pb) = pb {
        pb.finish_and_clear();
    }
    Ok(())
}
