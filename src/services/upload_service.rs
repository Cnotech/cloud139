use crate::client::ClientError;
use crate::domain::StorageType;
use indicatif::ProgressBar;
use std::path::Path;

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
    use super::{
        directory_service as dirs,
        sync_service::{SyncScanOptions, scan_local},
    };
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
    dirs::ensure_dir(config, &root, true).await?;
    for entry in entries {
        let dest = format!("{}/{}", root.trim_end_matches('/'), entry.rel_path);
        if entry.kind == crate::domain::SyncEntryKind::Directory {
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
