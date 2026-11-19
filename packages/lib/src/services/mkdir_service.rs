use super::directory_service as dirs;
use crate::client::api::personal::{ListingRetry, PersonalClient};
use crate::utils::remote_path;
use crate::{client::api, config::Config, error::ClientError, error::invalid};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MkdirMode {
    Single,
    Parents,
}

pub async fn mkdir(config: &Config, path: &str, mode: MkdirMode) -> Result<(), ClientError> {
    config.require_personal()?;
    let parents = mode == MkdirMode::Parents;
    let path = remote_path::normalize(path)?;
    if path == "/" {
        return if parents {
            Ok(())
        } else {
            Err(ClientError::CannotOperateOnRoot)
        };
    }
    let client = api::HttpClientWrapper::new();
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
