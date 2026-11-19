use super::directory_service as dirs;
use crate::{
    client::{ClientError, api},
    config::Config,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MkdirMode {
    Single,
    Parents,
}

pub async fn mkdir(config: &Config, path: &str, mode: MkdirMode) -> Result<(), ClientError> {
    config.require_personal()?;
    let parents = mode == MkdirMode::Parents;
    let path = dirs::normalize(path)?;
    if path == "/" {
        return if parents {
            Ok(())
        } else {
            Err(ClientError::CannotOperateOnRoot)
        };
    }
    let client = api::HttpClientWrapper::new();
    let listing = api::PersonalClient::connect(config, &client, api::ListingRetry::None).await?;
    dirs::ensure_personal_dir(&listing, &path, parents).await?;
    Ok(())
}

pub fn parse_path(path: &str) -> Result<(String, String), ClientError> {
    if path.trim().is_empty() {
        return Err(dirs::invalid("路径不能为空"));
    }
    let path = dirs::non_root(path.trim())?;
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
