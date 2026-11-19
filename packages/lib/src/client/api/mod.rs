pub mod headers;
pub(crate) mod personal;
mod response;
mod route;
pub mod upload;

use crate::client::http::HttpClientWrapper;
use crate::config::{Config, PersonalConfig};
use crate::debug;
use crate::error::ClientError;
use crate::models::common::QueryFileData;
use personal::{ListingRetry, PersonalClient};

pub async fn get_personal_file_detail(
    config: &Config,
    file_id: &str,
) -> Result<QueryFileData, ClientError> {
    let config = PersonalConfig::new(config)?;
    debug!("查询个人云文件详情 file_id={}", file_id);
    let client = HttpClientWrapper::new();
    let personal = PersonalClient::connect(config, &client, ListingRetry::None).await?;
    personal.file_detail(file_id).await
}

pub async fn get_file_id_by_path(config: &Config, path: &str) -> Result<String, ClientError> {
    let config = PersonalConfig::new(config)?;
    let normalized = crate::utils::remote_path::normalize(path)?;
    let path = normalized.as_str();
    if path == "/" {
        debug!("云端根路径返回空文件 ID");
        return Ok(String::new());
    }

    let client = HttpClientWrapper::new();
    let listing = PersonalClient::connect(config, &client, ListingRetry::None).await?;
    listing
        .resolve_path(path)
        .await?
        .and_then(|item| item.file_id)
        .ok_or(ClientError::InvalidFilePath)
}

pub async fn get_personal_download_link(
    config: &Config,
    file_id: &str,
) -> Result<String, ClientError> {
    let config = PersonalConfig::new(config)?;
    let client = HttpClientWrapper::new();
    let personal = PersonalClient::connect(config, &client, ListingRetry::None).await?;
    personal.download_link(file_id).await.map(|link| link.url)
}
