use crate::client::{ClientError, api};
use crate::domain::{FileEntry, PERSONAL_UPLOAD_MAX_BYTES, StorageType, SyncEntryKind};
use indicatif::ProgressBar;
use std::collections::HashMap;
use std::path::Path;

use super::directory_service as dirs;

pub fn validate_upload_size(storage: StorageType, size: u64) -> Result<(), ClientError> {
    if storage == StorageType::PersonalNew && size > PERSONAL_UPLOAD_MAX_BYTES {
        return Err(ClientError::UploadTooLarge {
            size,
            max_size: PERSONAL_UPLOAD_MAX_BYTES,
        });
    }
    Ok(())
}

/// Re-read metadata before hashing or any destructive sync operation.
pub fn checked_upload_size(storage: StorageType, path: &Path) -> Result<i64, ClientError> {
    dirs::safe_local(path)?;
    let metadata = std::fs::metadata(path)?;
    if !metadata.is_file() {
        return Err(dirs::invalid("只支持普通文件；目录请使用递归上传"));
    }
    validate_upload_size(storage, metadata.len())?;
    i64::try_from(metadata.len()).map_err(|_| dirs::invalid("文件过大"))
}

pub struct UploadPartParams<'a> {
    pub upload_url: &'a str,
    pub upload_task_id: &'a str,
    pub buffer: &'a [u8],
    pub part_number: i64,
    pub part_offset: i64,
    pub read_size: i64,
    pub file_name: &'a str,
    pub total_size: i64,
}

pub fn get_part_size(size: i64, custom_size: i64) -> i64 {
    if custom_size != 0 {
        return custom_size;
    }
    if size / (1024 * 1024 * 1024) > 30 {
        return 512 * 1024 * 1024;
    }
    100 * 1024 * 1024
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
    super::directory_service::validate_name(file_name)?;
    super::directory_service::safe_local(local_path)?;
    if !local_path.is_file() {
        return Err(super::directory_service::invalid(
            "只支持普通文件；目录请使用递归上传",
        ));
    }
    validate_upload_size(
        config.storage_type(),
        file_size
            .try_into()
            .map_err(|_| dirs::invalid("无效文件大小"))?,
    )?;
    let file_size = checked_upload_size(config.storage_type(), local_path)?;
    let remote_path = super::directory_service::normalize(remote_path)?;
    let parent = super::directory_service::directory(config, &remote_path).await?;
    let storage_type = config.storage_type();
    let conflicts = if storage_type == StorageType::PersonalNew {
        crate::client::api::list_personal_files(config, &parent)
            .await?
            .into_iter()
            .filter(|i| i.name.as_deref() == Some(file_name))
            .map(|i| super::sync_service::should_treat_personal_item_as_folder(&i))
            .collect::<Vec<_>>()
    } else {
        let dir = super::legacy_directory::resolve(config, &remote_path).await?;
        super::legacy_directory::list_all(config, &dir)
            .await?
            .into_iter()
            .filter(|i| i.name == file_name)
            .map(|i| i.folder)
            .collect()
    };
    if !conflicts.is_empty()
        && (conflicts.iter().any(|folder| *folder)
            || !force
            || storage_type != StorageType::PersonalNew)
    {
        return Err(super::directory_service::invalid(
            "云端已有同名条目；目录冲突禁止覆盖，家庭/群组不支持强制覆盖",
        ));
    }
    let remote_path = remote_path.as_str();
    match storage_type {
        StorageType::PersonalNew => {
            crate::services::upload::personal::upload(
                config,
                local_path,
                remote_path,
                file_name,
                file_size,
                force,
                pb,
            )
            .await
        }
        StorageType::Family => {
            crate::services::upload::family::upload(
                config,
                local_path,
                remote_path,
                file_name,
                file_size,
                pb,
            )
            .await
        }
        StorageType::Group => {
            crate::services::upload::group::upload(
                config,
                local_path,
                remote_path,
                file_name,
                file_size,
                pb,
            )
            .await
        }
    }
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
    dirs::safe_local(local)?;
    if !local.is_dir() {
        return Err(dirs::invalid("递归上传源必须是目录").into());
    }
    let mut entries = scan_local(
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
    if config.storage_type() == StorageType::PersonalNew {
        let mut oversized = 0;
        entries.retain(|entry| {
            if entry.kind == SyncEntryKind::File
                && let Err(error) = validate_upload_size(StorageType::PersonalNew, entry.size)
            {
                crate::warn!("超限未传: {}: {}", entry.rel_path, error);
                oversized += 1;
                return false;
            }
            true
        });
        return upload_personal_directory(config, local, &root, entries, force, oversized).await;
    }
    dirs::ensure_dir(config, &root, true).await?;
    for entry in entries {
        let dest = format!("{}/{}", root.trim_end_matches('/'), entry.rel_path);
        if entry.kind == SyncEntryKind::Directory {
            dirs::ensure_dir(config, &dest, true).await?;
        } else {
            let (parent, name) = dest.rsplit_once('/').unwrap();
            dirs::ensure_dir(config, parent, true).await?;
            let local_file = local.join(&entry.rel_path);
            dirs::safe_local(&local_file)?;
            upload(
                config,
                &local_file,
                parent,
                name,
                i64::try_from(entry.size).map_err(|_| dirs::invalid("文件过大"))?,
                force,
                None,
            )
            .await
            .map_err(|e| {
                anyhow::anyhow!("上传 {} 失败；此前条目可能已完成: {}", entry.rel_path, e)
            })?;
        }
    }
    Ok(())
}

async fn upload_personal_directory(
    config: &crate::config::Config,
    local: &Path,
    root: &str,
    entries: Vec<FileEntry>,
    force: bool,
    mut oversized: usize,
) -> anyhow::Result<()> {
    if root == "/" && entries.is_empty() && oversized == 0 {
        return Ok(());
    }
    let mut config = config.clone();
    let http_client = api::HttpClientWrapper::new();
    let host = api::get_personal_cloud_host_with_client(&mut config, &http_client).await?;
    let listing = api::PersonalListing {
        config: &config,
        host: &host,
        http_client: &http_client,
        retry_transient: true,
    };
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
        let result =
            upload_personal_directory_file(&listing, &path, parent_id, name, entry.size, force)
                .await;
        match result {
            Ok(()) => uploaded += 1,
            Err(error @ ClientError::UploadTooLarge { .. }) => {
                oversized += 1;
                crate::warn!("超限未传: {}: {}", entry.rel_path, error);
            }
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
    if oversized > 0 || !failures.is_empty() {
        anyhow::bail!(
            "目录上传未完成: 成功 {} 个，超限未传 {} 个，其他失败 {} 个{}",
            uploaded,
            oversized,
            failures.len(),
            if failures.is_empty() {
                String::new()
            } else {
                format!("；{}", failures.join("；"))
            }
        );
    }
    crate::success!("目录上传完成: 成功 {} 个文件", uploaded);
    Ok(())
}

async fn upload_personal_directory_file(
    listing: &api::PersonalListing<'_>,
    path: &Path,
    parent_id: &str,
    name: &str,
    size: u64,
    force: bool,
) -> Result<(), ClientError> {
    dirs::safe_local(path)?;
    if !path.is_file() {
        return Err(dirs::invalid("只支持普通文件；目录请使用递归上传"));
    }
    validate_upload_size(StorageType::PersonalNew, size)?;
    let size = checked_upload_size(StorageType::PersonalNew, path)?;
    let items = listing.list_files(parent_id).await?;
    if items.iter().any(|item| {
        item.name.as_deref() == Some(name)
            && (!force || super::sync_service::should_treat_personal_item_as_folder(item))
    }) {
        return Err(dirs::invalid(
            "云端已有同名条目；目录冲突禁止覆盖，家庭/群组不支持强制覆盖",
        ));
    }
    super::upload::personal::upload_to_parent(
        listing.config,
        listing.host,
        path,
        parent_id,
        name,
        size,
        None,
    )
    .await
}
