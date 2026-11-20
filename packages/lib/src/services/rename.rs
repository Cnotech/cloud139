//! 在原目录中重命名云端条目。

use super::directory as dirs;
use crate::client::api::personal::client::{ListingRetry, PersonalClient};
use crate::client::http::HttpClientWrapper;
use crate::config::{Config, personal::PersonalConfig};
use crate::error::ClientError;
use crate::utils::remote_path;

/// 在原目录中将 `source` 重命名为 `target`。`target` 是单个名称。
///
/// `force` 允许在两个文件同名时提交重命名。涉及目录的同名冲突会报错。
pub async fn rename(
    config: &Config,
    source: &str,
    target: &str,
    force: bool,
) -> Result<(), ClientError> {
    let config = PersonalConfig::new(config)?;
    let source = remote_path::non_root(source)?;
    remote_path::validate_name(target)?;
    let client = HttpClientWrapper::new();
    let personal = PersonalClient::connect(config, &client, ListingRetry::None).await?;
    if source.rsplit('/').next() == Some(target) {
        dirs::resolve_entry(&personal, &source).await?;
        return Ok(());
    }
    let (entry, siblings) = personal.resolve_path_with_siblings(&source).await?;
    let entry = entry.ok_or(ClientError::FileNotFound)?;
    if let Some(conflict) = siblings
        .iter()
        .find(|item| item.name.as_deref() == Some(target))
        && (entry.is_folder() || conflict.is_folder() || !force)
    {
        return Err(ClientError::ForceRequired);
    }
    let id = entry.file_id.ok_or(ClientError::InvalidFilePath)?;
    personal.rename(&id, target).await
}
