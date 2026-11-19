use crate::models::sync_item::{SyncDirection, SyncEndpoint};
use crate::utils::remote_path;
use anyhow::{Result, anyhow};
use std::path::{Path, PathBuf};

pub fn parse_sync_endpoint(value: &str) -> SyncEndpoint {
    if let Some(rest) = value.strip_prefix("cloud:") {
        return SyncEndpoint::Cloud(normalize_cloud_path(rest));
    }
    SyncEndpoint::Local(PathBuf::from(value))
}

/// 统一同步端点中的云端路径首尾斜杠，路径校验由同步入口负责。
pub fn normalize_cloud_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() || trimmed == "/" {
        return "/".to_string();
    }
    format!("/{}", trimmed.trim_start_matches('/').trim_end_matches('/'))
}

pub(super) fn normalize_sync_endpoint(endpoint: SyncEndpoint) -> Result<SyncEndpoint> {
    match endpoint {
        SyncEndpoint::Cloud(path) => Ok(SyncEndpoint::Cloud(remote_path::normalize(&path)?)),
        local => Ok(local),
    }
}

pub(super) fn rel_to_local(root: &Path, rel_path: &str) -> PathBuf {
    rel_path
        .split('/')
        .filter(|part| !part.is_empty())
        .fold(root.to_path_buf(), |path, part| path.join(part))
}

pub fn sync_direction(src: &SyncEndpoint, dest: &SyncEndpoint) -> Result<SyncDirection> {
    match (src, dest) {
        (SyncEndpoint::Local(_), SyncEndpoint::Cloud(_)) => Ok(SyncDirection::LocalToCloud),
        (SyncEndpoint::Cloud(_), SyncEndpoint::Local(_)) => Ok(SyncDirection::CloudToLocal),
        (SyncEndpoint::Local(_), SyncEndpoint::Local(_)) => {
            Err(anyhow!("SRC 和 DEST 都是本地路径，请使用 cp/mv 或系统工具"))
        }
        (SyncEndpoint::Cloud(_), SyncEndpoint::Cloud(_)) => {
            Err(anyhow!("SRC 和 DEST 都是云端路径，请使用 cp/mv 命令"))
        }
    }
}

pub(super) const SYNC_UPLOAD_PREFIX: &str = ".cloud139-upload-";
pub(super) const UPLOAD_BACKUP_PREFIX: &str = ".cloud139-backup-";

pub(super) fn is_recovery_name(name: &str) -> bool {
    name.starts_with(SYNC_UPLOAD_PREFIX) || name.starts_with(UPLOAD_BACKUP_PREFIX)
}

pub(super) fn is_recovery_path(path: &str) -> bool {
    path.split('/').any(is_recovery_name)
}
