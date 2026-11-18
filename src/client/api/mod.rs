use self::endpoints::ROUTE_POLICY_URL;
use self::request::{SignedEndpoint, signed_post};
use crate::client::ClientError;
use crate::config::Config;
use crate::debug;
use crate::models::QueryRoutePolicyResp;

pub mod api_trait;
pub mod endpoints;
pub mod headers;
mod personal;
pub(super) mod refresh;
mod request;
mod transfer;
pub mod upload;

pub use personal::mutation_succeeded;
pub(crate) use personal::{ListPageOptions, ListingRetry, PersonalClient};
pub use request::HttpClientWrapper;
pub(super) use request::desktop_http_client;
pub(crate) use transfer::DownloadLink;

pub async fn get_personal_cloud_host(config: &mut Config) -> Result<String, ClientError> {
    get_personal_cloud_host_with_client(config, &HttpClientWrapper::new()).await
}

pub async fn get_personal_cloud_host_with_client(
    config: &mut Config,
    http_client: &HttpClientWrapper,
) -> Result<String, ClientError> {
    config.require_personal()?;
    if let Some(ref host) = config.personal_cloud_host {
        debug!("使用缓存的个人云地址 host={}", host);
        return Ok(host.clone());
    }

    let config_path = config.cache_path();
    let host = fetch_personal_cloud_host(config, http_client).await?;
    config.personal_cloud_host = Some(host.clone());
    if let Some(path) = config_path {
        let _ = config.update_cache(&path, |current| {
            current.personal_cloud_host = Some(host.clone());
        });
    }
    Ok(host)
}

pub(crate) async fn fetch_personal_cloud_host(
    config: &Config,
    http_client: &HttpClientWrapper,
) -> Result<String, ClientError> {
    config.require_personal()?;
    let url = ROUTE_POLICY_URL;
    debug!("查询个人云路由 url={}", url);

    let body = serde_json::json!({
        "userInfo": {
            "userType": 1,
            "accountType": 1,
            "accountName": config.account
        },
        "modAddrType": 1
    });

    let route_resp: QueryRoutePolicyResp = signed_post(
        &config.authorization,
        url,
        &body,
        &http_client.client,
        SignedEndpoint::Route,
    )
    .await?;

    let host = route_resp
        .data
        .route_policy_list
        .into_iter()
        .find(|p| p.mod_name.as_deref() == Some("personal"))
        .map(|p| p.https_url.unwrap_or_default())
        .ok_or_else(|| ClientError::Other("未找到个人云地址".into()))?;

    debug!("已解析个人云地址 host={}", host);
    Ok(host)
}

pub async fn get_file_id_by_path(config: &Config, path: &str) -> Result<String, ClientError> {
    config.require_personal()?;
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

pub fn parse_path_segments(path: &str) -> Vec<&str> {
    path.trim_start_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .collect()
}

pub fn get_parent_id(current_parent_id: &str) -> String {
    if current_parent_id.is_empty() {
        "/".to_string()
    } else {
        current_parent_id.to_string()
    }
}

pub async fn personal_api_request<T: for<'de> serde::Deserialize<'de>>(
    config: &Config,
    url: &str,
    body: serde_json::Value,
    storage_type: crate::domain::StorageType,
) -> Result<T, ClientError> {
    personal_api_request_with_client(config, url, body, storage_type, &HttpClientWrapper::new())
        .await
}

pub async fn personal_api_request_with_client<T: for<'de> serde::Deserialize<'de>>(
    config: &Config,
    url: &str,
    body: serde_json::Value,
    storage_type: crate::domain::StorageType,
    http_client: &HttpClientWrapper,
) -> Result<T, ClientError> {
    personal_post(config, url, &body, storage_type, &http_client.client).await
}

pub(super) async fn personal_post<T: for<'de> serde::Deserialize<'de>>(
    config: &Config,
    url: &str,
    body: &serde_json::Value,
    storage_type: crate::domain::StorageType,
    client: &reqwest::Client,
) -> Result<T, ClientError> {
    config.require_personal()?;
    if storage_type != crate::domain::StorageType::PersonalNew {
        return Err(ClientError::UnsupportedStorage);
    }
    signed_post(
        &config.authorization,
        url,
        body,
        client,
        SignedEndpoint::Personal,
    )
    .await
}

pub async fn check_file_exists(
    config: &Config,
    parent_file_id: &str,
    file_name: &str,
) -> Result<bool, ClientError> {
    check_file_exists_with_client(config, parent_file_id, file_name, &HttpClientWrapper::new())
        .await
}

pub async fn check_file_exists_with_client(
    config: &Config,
    parent_file_id: &str,
    file_name: &str,
    http_client: &HttpClientWrapper,
) -> Result<bool, ClientError> {
    debug!(
        "检查云端文件是否存在 parent={} name={}",
        parent_file_id, file_name
    );
    let personal = PersonalClient::connect(config, http_client, ListingRetry::None).await?;
    let exists = personal
        .find_child(parent_file_id, file_name)
        .await?
        .is_some();
    debug!("已检查云端文件是否存在 result={}", exists);
    Ok(exists)
}

pub async fn list_personal_files(
    config: &Config,
    parent_file_id: &str,
) -> Result<Vec<crate::models::PersonalFileItem>, ClientError> {
    list_personal_files_with_client(config, parent_file_id, &HttpClientWrapper::new()).await
}

pub async fn list_personal_files_with_client(
    config: &Config,
    parent_file_id: &str,
    http_client: &HttpClientWrapper,
) -> Result<Vec<crate::models::PersonalFileItem>, ClientError> {
    PersonalClient::connect(config, http_client, ListingRetry::None)
        .await?
        .list_files(parent_file_id)
        .await
}

pub async fn get_family_download_link(
    _config: &Config,
    _content_id: &str,
    _path: &str,
) -> Result<String, ClientError> {
    Err(ClientError::UnsupportedStorage)
}

pub async fn get_group_download_link(
    _config: &Config,
    _content_id: &str,
    _path: &str,
) -> Result<String, ClientError> {
    Err(ClientError::UnsupportedStorage)
}

pub async fn get_family_root_path(_config: &Config) -> Result<String, ClientError> {
    Err(ClientError::UnsupportedStorage)
}

pub async fn get_group_root_by_cloud_id(_config: &Config) -> Result<String, ClientError> {
    Err(ClientError::UnsupportedStorage)
}

pub async fn get_personal_file_detail(
    config: &Config,
    file_id: &str,
) -> Result<crate::models::QueryFileData, ClientError> {
    debug!("查询个人云文件详情 file_id={}", file_id);
    let client = HttpClientWrapper::new();
    let personal = PersonalClient::connect(config, &client, ListingRetry::None).await?;
    personal.file_detail(file_id).await
}

pub async fn get_personal_download_link(
    config: &Config,
    file_id: &str,
) -> Result<String, ClientError> {
    let client = HttpClientWrapper::new();
    let personal = PersonalClient::connect(config, &client, ListingRetry::None).await?;
    personal.download_link(file_id).await.map(|link| link.url)
}
