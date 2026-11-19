use super::directory as dirs;
use crate::client::api::personal::{ListingRetry, PersonalClient};
use crate::client::http::HttpClientWrapper;
use crate::config::{Config, PersonalConfig};
use crate::error::{ClientError, invalid};
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

pub fn parse_path(path: &str) -> Result<(String, String), ClientError> {
    if path.trim().is_empty() {
        return Err(invalid("路径不能为空"));
    }
    let path = remote_path::non_root(path.trim())?;
    let (parent, name) = path.rsplit_once('/').unwrap();
    Ok((
        if parent.is_empty() {
            "/".into()
        } else {
            parent.into()
        },
        name.into(),
    ))
}
