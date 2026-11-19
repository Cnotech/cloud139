mod directory;
pub(super) mod transfer;

use self::directory::upload_personal_tree;
use self::transfer::transfer_to_parent;
use super::directory as dirs;
use crate::client::api::personal::client::{ListingRetry, PersonalClient};
use crate::client::http::HttpClientWrapper;
use crate::config::{Config, personal::PersonalConfig};
use crate::error::{ClientError, invalid};
use crate::progress::Progress;
use crate::utils::{path::safe_local, remote_path};
use std::path::Path;

pub async fn upload_path(
    config: &Config,
    local: &str,
    remote: &str,
    force: bool,
    make_progress: impl FnOnce(&str, u64) -> Option<Progress>,
) -> anyhow::Result<()> {
    let config = PersonalConfig::new(config)?;
    let local_path = Path::new(local);
    if !local_path.exists() {
        crate::error!("文件不存在: {}", local);
        return Err(ClientError::FileNotFound.into());
    }

    safe_local(local_path)?;
    if local_path.is_dir() {
        return upload_personal_tree(
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

    upload_personal_file(
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

/// 计算哈希或执行可能删除、替换文件的同步操作前，重新读取元数据。
pub fn checked_upload_size(path: &Path) -> Result<i64, ClientError> {
    safe_local(path)?;
    let metadata = std::fs::metadata(path)?;
    if !metadata.is_file() {
        return Err(invalid("只支持普通文件；目录请使用递归上传"));
    }
    i64::try_from(metadata.len()).map_err(|_| invalid("文件过大"))
}

pub async fn upload(
    config: &Config,
    local_path: &Path,
    remote_path: &str,
    file_name: &str,
    file_size: i64,
    force: bool,
    pb: Option<Progress>,
) -> Result<(), ClientError> {
    upload_personal_file(
        PersonalConfig::new(config)?,
        local_path,
        remote_path,
        file_name,
        file_size,
        force,
        pb,
    )
    .await
}

async fn upload_personal_file(
    config: PersonalConfig<&Config>,
    local_path: &Path,
    remote_path: &str,
    file_name: &str,
    file_size: i64,
    force: bool,
    pb: Option<Progress>,
) -> Result<(), ClientError> {
    remote_path::validate_name(file_name)?;
    if file_size < 0 {
        return Err(invalid("无效文件大小"));
    }
    checked_upload_size(local_path)?;
    let remote_path = remote_path::normalize(remote_path)?;
    let http_client = HttpClientWrapper::new();
    let listing = PersonalClient::connect(config, &http_client, ListingRetry::DIRECTORY).await?;
    let parent = dirs::directory_with_listing(&listing, &remote_path).await?;
    upload_file(&listing, local_path, &parent, file_name, force, pb).await
}

/// 上传 `dir` 目录本身。若本地路径以分隔符结尾，则只上传目录内容。
pub async fn upload_directory(
    config: &Config,
    local: &Path,
    remote: &str,
    contents: bool,
    force: bool,
) -> anyhow::Result<()> {
    upload_personal_tree(PersonalConfig::new(config)?, local, remote, contents, force).await
}

async fn upload_file(
    listing: &PersonalClient<'_>,
    path: &Path,
    parent_id: &str,
    name: &str,
    force: bool,
    pb: Option<Progress>,
) -> Result<(), ClientError> {
    let size = checked_upload_size(path)?;
    let items = listing.list_files(parent_id).await?;
    for item in items
        .iter()
        .filter(|item| item.name.as_deref() == Some(name))
    {
        if item.is_folder() {
            return Err(invalid("云端同名目录阻止上传文件"));
        }
        if !force {
            return Err(ClientError::ForceRequired);
        }
    }
    let saved_name = upload_to_parent(listing, path, parent_id, name, size, pb.clone()).await?;
    finish_upload(&saved_name, &pb);
    Ok(())
}

async fn upload_to_parent(
    personal: &PersonalClient<'_>,
    local_path: &std::path::Path,
    parent_file_id: &str,
    file_name: &str,
    file_size: i64,
    pb: Option<Progress>,
) -> Result<String, ClientError> {
    let uploaded = transfer_to_parent(
        personal,
        local_path,
        parent_file_id,
        file_name,
        file_size,
        pb,
    )
    .await?;
    if let Some(name) = uploaded.name {
        return Ok(name);
    }
    let files = personal
        .with_listing_retry(ListingRetry::None)
        .list_files(parent_file_id)
        .await?;
    files
        .into_iter()
        .find(|item| item.file_id.as_deref() == Some(&uploaded.id) && !item.is_folder())
        .and_then(|item| item.name)
        .ok_or_else(|| ClientError::Api("上传已提交，但无法确认云端文件名称".into()))
}

pub(super) fn finish_upload(name: &str, pb: &Option<Progress>) {
    crate::success!("上传完成: {name}");
    if let Some(bar) = pb {
        bar.finish();
    }
}
