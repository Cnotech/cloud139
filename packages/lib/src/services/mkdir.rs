use super::directory as dirs;
use crate::client::api::personal::client::{ListingRetry, PersonalClient};
use crate::client::http::HttpClientWrapper;
use crate::config::{Config, personal::PersonalConfig};
use crate::error::ClientError;
use crate::utils::remote_path;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MkdirMode {
    Single,
    Parents,
}

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
