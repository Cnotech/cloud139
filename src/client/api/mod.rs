use crate::client::ClientError;
use crate::client::endpoints::ROUTE_POLICY_URL;
use crate::client::headers::{build_route_headers, build_signed_headers};
use crate::config::Config;
use crate::debug;
use crate::models::QueryRoutePolicyResp;
use crate::utils::generate_rand_str;
use std::time::Duration;

mod personal;
pub(crate) use personal::{ListPageOptions, ListingRetry, PersonalClient};

pub struct HttpClientWrapper {
    pub client: reqwest::Client,
}

impl Default for HttpClientWrapper {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpClientWrapper {
    pub fn new() -> Self {
        Self {
            client: http_client_builder()
                .build()
                .expect("HTTP 客户端初始化失败"),
        }
    }

    pub fn with_client(client: reqwest::Client) -> Self {
        Self { client }
    }
}

pub(super) fn http_client_builder() -> reqwest::ClientBuilder {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .read_timeout(Duration::from_secs(30))
}

pub async fn get_personal_cloud_host(config: &mut Config) -> Result<String, ClientError> {
    get_personal_cloud_host_with_client(config, &HttpClientWrapper::new()).await
}

pub async fn get_personal_cloud_host_with_client(
    config: &mut Config,
    http_client: &HttpClientWrapper,
) -> Result<String, ClientError> {
    config.require_personal()?;
    if let Some(ref host) = config.personal_cloud_host {
        debug!("get_personal_cloud_host: 使用缓存 host={}", host);
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
    debug!("get_personal_cloud_host: 请求 url={}", url);

    let body = serde_json::json!({
        "userInfo": {
            "userType": 1,
            "accountType": 1,
            "accountName": config.account
        },
        "modAddrType": 1
    });

    let route_resp: QueryRoutePolicyResp = signed_post(
        config,
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
        .ok_or_else(|| ClientError::Other("Could not find personal cloud host".to_string()))?;

    debug!("get_personal_cloud_host: 解析到 host={}", host);
    Ok(host)
}

pub async fn get_file_id_by_path(config: &Config, path: &str) -> Result<String, ClientError> {
    config.require_personal()?;
    let normalized = crate::utils::remote_path::normalize(path)?;
    let path = normalized.as_str();
    if path == "/" {
        debug!("get_file_id_by_path: 根路径返回空ID");
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
    signed_post(config, url, body, client, SignedEndpoint::Personal).await
}

enum SignedEndpoint {
    Route,
    Personal,
}

async fn signed_post<T: for<'de> serde::Deserialize<'de>>(
    config: &Config,
    url: &str,
    body: &serde_json::Value,
    client: &reqwest::Client,
    endpoint: SignedEndpoint,
) -> Result<T, ClientError> {
    let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let nonce = generate_rand_str(16);
    let payload = body.to_string();
    let sign = crate::utils::crypto::calc_sign(&payload, &ts, &nonce);
    let auth = &config.authorization;
    let mut headers = match endpoint {
        SignedEndpoint::Route => build_route_headers(auth, &ts, &nonce, &sign),
        SignedEndpoint::Personal => build_signed_headers(auth, &ts, &nonce, &sign, "1"),
    }?;
    headers.entry(reqwest::header::CONTENT_TYPE).or_insert(
        reqwest::header::HeaderValue::from_static("application/json"),
    );

    debug!("POST {}", url);
    Ok(client
        .post(url)
        .headers(headers)
        .body(payload)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?)
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
        "check_file_exists: parent={}, name={}",
        parent_file_id, file_name
    );
    let personal = PersonalClient::connect(config, http_client, ListingRetry::None).await?;
    let exists = personal
        .find_child(parent_file_id, file_name)
        .await?
        .is_some();
    debug!("check_file_exists: result={}", exists);
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
    debug!("get_personal_file_detail: file_id={}", file_id);
    let client = HttpClientWrapper::new();
    let personal = PersonalClient::connect(config, &client, ListingRetry::None).await?;
    let resp: crate::models::QueryFileResp = personal
        .request("/file/query", serde_json::json!({ "fileId": file_id }))
        .await?;

    if !resp.base.success {
        let msg = resp
            .base
            .message
            .unwrap_or_else(|| "获取文件详情失败".to_string());
        return Err(ClientError::Api(msg));
    }

    Ok(resp.data)
}

pub async fn get_personal_download_link(
    config: &Config,
    file_id: &str,
) -> Result<String, ClientError> {
    let client = HttpClientWrapper::new();
    let personal = PersonalClient::connect(config, &client, ListingRetry::None).await?;
    personal.download_link(file_id).await.map(|(url, _)| url)
}
