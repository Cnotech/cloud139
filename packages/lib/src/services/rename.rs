use super::directory as dirs;
use crate::client::api::personal::client::{ListingRetry, PersonalClient};
use crate::client::http::HttpClientWrapper;
use crate::config::{Config, PersonalConfig};
use crate::error::ClientError;
use crate::utils::remote_path;
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
    let (entry, _) = dirs::resolve_entry(&personal, &source).await?;
    let (parent, name) = source.rsplit_once('/').unwrap();
    if name == target {
        return Ok(());
    }
    let parent_id = dirs::directory_with_listing(&personal, parent).await?;
    if let Some(conflict) = personal
        .list_files(&parent_id)
        .await?
        .iter()
        .find(|item| item.name.as_deref() == Some(target))
        && (entry.folder || conflict.is_folder() || !force)
    {
        return Err(ClientError::ForceRequired);
    }
    personal.rename(&entry.id, target).await
}
