use crate::client::ClientError;
use crate::client::endpoints::ROUTE_POLICY_URL;
use crate::client::headers::{build_route_headers, build_signed_headers};
use crate::config::Config;
use crate::debug;
use crate::models::QueryRoutePolicyResp;
use crate::utils::generate_rand_str;

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
            client: reqwest::Client::builder()
                .connect_timeout(std::time::Duration::from_secs(10))
                .read_timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("HTTP 客户端初始化失败"),
        }
    }

    pub fn with_client(client: reqwest::Client) -> Self {
        Self { client }
    }
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

    let client = &http_client.client;

    let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let rand_str = generate_rand_str(16);
    let body_str = body.to_string();
    let sign = crate::utils::crypto::calc_sign(&body_str, &ts, &rand_str);

    let headers = build_route_headers(&config.authorization, &ts, &rand_str, &sign)?;

    let resp = client
        .post(url)
        .headers(headers)
        .json(&body)
        .send()
        .await?
        .error_for_status()?;

    let route_resp: QueryRoutePolicyResp = resp.json().await?;

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

    debug!("get_file_id_by_path: 解析路径={}", path);

    let parts: Vec<&str> = path
        .trim_start_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();

    // host 只解析一次：逐层解析时不再重复请求路由（缓存未命中时省掉 N-1 次请求）。
    let mut config = config.clone();
    let host = get_personal_cloud_host(&mut config).await?;
    let http_client = HttpClientWrapper::new();
    let mut current_parent_id = String::new();

    for (i, part) in parts.iter().enumerate() {
        let is_last = i == parts.len() - 1;
        let parent_id = if current_parent_id.is_empty() {
            "/".to_string()
        } else {
            current_parent_id.clone()
        };

        // 按页查找，命中即停：不再把父目录所有页取完再找。第一页命中时，
        // 后续页的失败不会拖垮本来可以解析的路径。
        let target_id = find_personal_child(&config, &host, &parent_id, part, &http_client)
            .await?
            .map(|item| {
                if !is_last && !item.is_folder() {
                    return Err(ClientError::InvalidFilePath);
                }
                item.file_id
                    .filter(|id| !id.is_empty())
                    .ok_or(ClientError::InvalidFilePath)
            })
            .transpose()?;

        match target_id {
            Some(id) => {
                if is_last {
                    debug!("get_file_id_by_path: 路径解析成功 path={}, id={}", path, id);
                    return Ok(id);
                }
                current_parent_id = id;
            }
            None => {
                return Err(ClientError::Api(format!(
                    "File or directory not found: {}",
                    part
                )));
            }
        }
    }

    Ok(current_parent_id)
}

/// 在一个父目录下按页查找子项，命中就返回，不再请求后续页。
pub(crate) async fn find_personal_child(
    config: &Config,
    host: &str,
    parent_file_id: &str,
    name: &str,
    http_client: &HttpClientWrapper,
) -> Result<Option<crate::models::PersonalFileItem>, ClientError> {
    PersonalListing {
        config,
        host,
        http_client,
        retry_transient: false,
    }
    .find_child(parent_file_id, name)
    .await
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
    config.require_personal()?;
    if storage_type != crate::domain::StorageType::PersonalNew {
        return Err(ClientError::UnsupportedStorage);
    }
    let svctype = "1";
    let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let rand_str = generate_rand_str(16);
    let body_str = body.to_string();
    let sign = crate::utils::crypto::calc_sign(&body_str, &ts, &rand_str);

    let client = &http_client.client;

    let headers = build_signed_headers(&config.authorization, &ts, &rand_str, &sign, svctype)?;

    debug!("personal_api_request: POST {} svctype={}", url, svctype);
    let resp = client
        .post(url)
        .headers(headers)
        .json(&body)
        .send()
        .await?
        .error_for_status()?;

    let result: T = resp.json().await?;
    Ok(result)
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
    let files = list_personal_files_with_client(config, parent_file_id, http_client).await?;
    let exists = files.iter().any(|f| f.name.as_deref() == Some(file_name));
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
    debug!("list_personal_files: parent_file_id={}", parent_file_id);
    let mut config = config.clone();
    let host = get_personal_cloud_host(&mut config).await?;
    PersonalListing {
        config: &config,
        host: &host,
        http_client,
        retry_transient: false,
    }
    .list_files(parent_file_id)
    .await
}

pub(crate) struct PersonalListing<'a> {
    pub config: &'a Config,
    pub host: &'a str,
    pub http_client: &'a HttpClientWrapper,
    pub retry_transient: bool,
}

impl PersonalListing<'_> {
    pub async fn find_child(
        &self,
        parent_id: &str,
        name: &str,
    ) -> Result<Option<crate::models::PersonalFileItem>, ClientError> {
        let items = self.read_items(parent_id, Some(name)).await?;
        Ok(items.into_iter().next())
    }

    pub async fn list_files(
        &self,
        parent_id: &str,
    ) -> Result<Vec<crate::models::PersonalFileItem>, ClientError> {
        self.read_items(parent_id, None).await
    }

    async fn read_items(
        &self,
        parent_id: &str,
        name: Option<&str>,
    ) -> Result<Vec<crate::models::PersonalFileItem>, ClientError> {
        let mut items = Vec::new();
        let mut cursor = String::new();
        let mut seen = std::collections::HashSet::new();
        loop {
            if !seen.insert(cursor.clone()) {
                return Err(ClientError::Api("分页游标重复，已停止列目录".into()));
            }
            let page = self.page(parent_id, &cursor).await?;
            if let Some(name) = name {
                if let Some(item) = page
                    .items
                    .into_iter()
                    .find(|item| item.name.as_deref() == Some(name))
                {
                    return Ok(vec![item]);
                }
            } else {
                items.extend(page.items);
            }
            cursor = page.next_page_cursor.unwrap_or_default();
            if cursor.is_empty() {
                return Ok(items);
            }
        }
    }

    async fn page(
        &self,
        parent_id: &str,
        cursor: &str,
    ) -> Result<crate::models::PersonalListData, ClientError> {
        let url = format!("{}/file/list", self.host);
        let body = serde_json::json!({
            "imageThumbnailStyleList": ["Small", "Large"],
            "orderBy": "updated_at",
            "orderDirection": "DESC",
            "pageInfo": {
                "pageCursor": cursor,
                "pageSize": 100
            },
            "parentFileId": parent_id
        });
        let request = || {
            personal_api_request_with_client::<crate::models::PersonalListResp>(
                self.config,
                &url,
                body.clone(),
                crate::domain::StorageType::PersonalNew,
                self.http_client,
            )
        };
        let resp = if self.retry_transient {
            crate::utils::retry::with_retries(2, 1, || async {
                tokio::time::timeout(std::time::Duration::from_secs(20), request())
                    .await
                    .map_err(|error| std::io::Error::new(std::io::ErrorKind::TimedOut, error))?
                    .map_err(Into::into)
            })
            .await
            .map_err(|error| {
                error
                    .downcast::<ClientError>()
                    .unwrap_or_else(|error| ClientError::Other(format!("{error:#}")))
            })?
        } else {
            request().await?
        };
        if !resp.base.success {
            return Err(ClientError::Api(
                resp.base
                    .message
                    .unwrap_or_else(|| "获取目录列表失败".into()),
            ));
        }
        resp.data
            .ok_or_else(|| ClientError::Api("获取目录列表失败: 无数据".into()))
    }
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
    let mut config = config.clone();
    let host = get_personal_cloud_host(&mut config).await?;
    let url = format!("{}/file/query", host);

    let body = serde_json::json!({
        "fileId": file_id
    });

    let resp: crate::models::QueryFileResp =
        personal_api_request(&config, &url, body, crate::domain::StorageType::PersonalNew).await?;

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
    debug!("get_personal_download_link: file_id={}", file_id);
    let mut config = config.clone();
    let host = get_personal_cloud_host(&mut config).await?;
    let url = format!("{}/file/getDownloadUrl", host);

    let body = serde_json::json!({
        "fileId": file_id
    });

    let resp: serde_json::Value =
        personal_api_request(&config, &url, body, crate::domain::StorageType::PersonalNew).await?;

    let cdn_url = resp
        .pointer("/data/cdnUrl")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if !cdn_url.is_empty() {
        debug!(
            "get_personal_download_link: 使用CDN链接, url_len={}",
            cdn_url.len()
        );
        return Ok(cdn_url);
    }

    let url = resp
        .pointer("/data/url")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    debug!(
        "get_personal_download_link: 使用普通链接, url_len={}",
        url.len()
    );
    Ok(url)
}
