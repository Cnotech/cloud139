//! 创建云端目录。

use super::directory as dirs;
use crate::client::api::personal::client::{ListingRetry, PersonalClient};
use crate::client::http::HttpClientWrapper;
use crate::config::{Config, personal::PersonalConfig};
use crate::error::ClientError;
use crate::utils::remote_path;

/// 创建目录时如何处理父目录和已存在的目标目录。
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MkdirMode {
    /// 父目录必须已存在，且目标目录必须尚未创建。
    Single,
    /// 补齐缺失的父目录，目标目录已存在时也视为成功。
    Parents,
}

/// 按指定模式创建云端目录。同名文件会导致创建失败。
pub async fn mkdir(config: &Config, path: &str, mode: MkdirMode) -> Result<(), ClientError> {
    let config = PersonalConfig::new(config)?;
    let parents = mode == MkdirMode::Parents;
    let path = remote_path::normalize(path)?;
    if path == "/" {
        return if parents {
            Ok(())
        } else {
            Err(ClientError::CannotOperateOnRoot)
        };
    }
    let client = HttpClientWrapper::new();
    let listing = PersonalClient::connect(config, &client, ListingRetry::None).await?;
    dirs::ensure_personal_dir(&listing, &path, parents).await?;
    Ok(())
}
