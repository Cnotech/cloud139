//! 将云端条目移入回收站。

use super::directory as dirs;
use crate::client::api::personal::client::{ListingRetry, PersonalClient};
use crate::client::http::HttpClientWrapper;
use crate::config::{Config, personal::PersonalConfig};
use crate::error::invalid;
use crate::utils::remote_path;

/// 将云端文件或目录移入回收站。目录要求 `recursive` 为 `true`，根目录不可删除。
pub async fn delete(config: &Config, path: &str, recursive: bool) -> anyhow::Result<()> {
    let config = PersonalConfig::new(config)?;
    let path = remote_path::non_root(path)?;
    let client = HttpClientWrapper::new();
    let personal = PersonalClient::connect(config, &client, ListingRetry::None).await?;
    let (entry, _) = dirs::resolve_entry(&personal, &path).await?;
    if entry.folder && !recursive {
        return Err(invalid("删除目录需要显式指定 -r / --recursive").into());
    }
    personal.trash(&entry.id).await?;
    Ok(())
}
