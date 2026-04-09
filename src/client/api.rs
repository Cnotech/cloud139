use crate::client::ClientError;
use crate::client::{
    CLIENT_INFO, DEVICE_INFO, MCLOUD_CHANNEL, MCLOUD_CHANNEL_SRC, MCLOUD_CLIENT, MCLOUD_VERSION,
};
use crate::config::Config;
use crate::models::QueryRoutePolicyResp;

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
            client: reqwest::Client::new(),
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
    if let Some(ref host) = config.personal_cloud_host {
        return Ok(host.clone());
    }

    let url = "https://user-njs.yun.139.com/user/route/qryRoutePolicy";

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

    use reqwest::header::{HeaderMap, HeaderValue};

    let mut headers = HeaderMap::new();
    headers.insert(
        "Accept",
        HeaderValue::from_static("application/json, text/plain, */*"),
    );
    headers.insert(
        "Content-Type",
        HeaderValue::from_static("application/json;charset=UTF-8"),
    );
    headers.insert("mcloud-channel", HeaderValue::from_static(MCLOUD_CHANNEL));
    headers.insert("mcloud-client", HeaderValue::from_static(MCLOUD_CLIENT));
    headers.insert("mcloud-version", HeaderValue::from_static(MCLOUD_VERSION));
    headers.insert("Origin", HeaderValue::from_static("https://yun.139.com"));
    headers.insert("Referer", HeaderValue::from_static("https://yun.139.com/w/"));
    headers.insert("x-DeviceInfo", HeaderValue::from_static(DEVICE_INFO));
    headers.insert("x-huawei-channelSrc", HeaderValue::from_static(MCLOUD_CHANNEL_SRC));
    headers.insert("x-inner-ntwk", HeaderValue::from_static("2"));
    headers.insert("x-m4c-caller", HeaderValue::from_static("PC"));
    headers.insert("x-m4c-src", HeaderValue::from_static("10002"));
    headers.insert("Inner-Hcy-Router-Https", HeaderValue::from_static("1"));

    headers.insert(
        "Authorization",
        format!("Basic {}", config.authorization)
            .parse()
            .map_err(|e| ClientError::InvalidHeader(format!("{}", e)))?,
    );
    headers.insert(
        "mcloud-sign",
        format!("{},{},{}", ts, rand_str, sign)
            .parse()
            .map_err(|e| ClientError::InvalidHeader(format!("{}", e)))?,
    );
    headers.insert(
        "x-SvcType",
        "1"
            .parse()
            .map_err(|e| ClientError::InvalidHeader(format!("{}", e)))?,
    );

    let resp = client.post(url).headers(headers).json(&body).send().await?;

    let route_resp: QueryRoutePolicyResp = resp.json().await?;

    let host = route_resp
        .data
        .route_policy_list
        .into_iter()
        .find(|p| p.mod_name.as_deref() == Some("personal"))
        .map(|p| p.https_url.unwrap_or_default())
        .ok_or_else(|| ClientError::Other("Could not find personal cloud host".to_string()))?;

    config.personal_cloud_host = Some(host.clone());
    let _ = config.save();

    Ok(host)
}

pub async fn get_file_id_by_path(config: &Config, path: &str) -> Result<String, ClientError> {
    if path.is_empty() || path == "/" {
        return Ok(String::new());
    }

    let mut config = config.clone();
    let host = get_personal_cloud_host(&mut config).await?;

    let parts: Vec<&str> = path
        .trim_start_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();

    let mut current_parent_id = String::new();

    for (i, part) in parts.iter().enumerate() {
        let is_last = i == parts.len() - 1;
        let parent_id = if current_parent_id.is_empty() {
            "/".to_string()
        } else {
            current_parent_id.clone()
        };

        let url = format!("{}/file/list", host);

        let body = serde_json::json!({
            "parentFileId": parent_id,
            "pageInfo": {
                "pageCursor": "",
                "pageSize": 100
            },
            "orderBy": "updated_at",
            "orderDirection": "DESC"
        });

        let list_resp: crate::models::PersonalListResp =
            personal_api_request(&config, &url, body, crate::client::StorageType::PersonalNew)
                .await?;

        let items = list_resp.data.map(|d| d.items).unwrap_or_default();

        let target_id = items
            .into_iter()
            .find(|item| item.name.as_deref() == Some(part))
            .map(|item| item.file_id.unwrap_or_default());

        match target_id {
            Some(id) => {
                if is_last {
                    return Ok(id);
                }
                current_parent_id = id;
            }
            None => {
                return Err(ClientError::Api(format!("文件或目录不存在: {}", part)));
            }
        }
    }

    Ok(current_parent_id)
}

fn generate_rand_str(len: usize) -> String {
    use rand::Rng;
    const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut rng = rand::thread_rng();
    (0..len)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
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
    storage_type: crate::client::StorageType,
) -> Result<T, ClientError> {
    personal_api_request_with_client(config, url, body, storage_type, &HttpClientWrapper::new())
        .await
}

pub async fn personal_api_request_with_client<T: for<'de> serde::Deserialize<'de>>(
    config: &Config,
    url: &str,
    body: serde_json::Value,
    storage_type: crate::client::StorageType,
    http_client: &HttpClientWrapper,
) -> Result<T, ClientError> {
    let svctype = match storage_type {
        crate::client::StorageType::PersonalNew => "1",
        crate::client::StorageType::Family => "2",
        crate::client::StorageType::Group => "3",
    };

    let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let rand_str = generate_rand_str(16);
    let body_str = body.to_string();
    let sign = crate::utils::crypto::calc_sign(&body_str, &ts, &rand_str);

    let client = &http_client.client;

    use reqwest::header::{HeaderMap, HeaderValue};

    let mut headers = HeaderMap::new();
    headers.insert(
        "Accept",
        HeaderValue::from_static("application/json, text/plain, */*"),
    );
    headers.insert("Caller", HeaderValue::from_static("web"));
    headers.insert("CMS-DEVICE", HeaderValue::from_static("default"));
    headers.insert("mcloud-channel", HeaderValue::from_static(MCLOUD_CHANNEL));
    headers.insert("mcloud-client", HeaderValue::from_static(MCLOUD_CLIENT));
    headers.insert("mcloud-route", HeaderValue::from_static("001"));
    headers.insert("mcloud-version", HeaderValue::from_static(MCLOUD_VERSION));
    headers.insert(
        "x-DeviceInfo",
        HeaderValue::from_static(DEVICE_INFO),
    );
    headers.insert("x-huawei-channelSrc", HeaderValue::from_static(MCLOUD_CHANNEL_SRC));
    headers.insert("x-inner-ntwk", HeaderValue::from_static("2"));
    headers.insert("x-m4c-caller", HeaderValue::from_static("PC"));
    headers.insert("x-m4c-src", HeaderValue::from_static("10002"));
    headers.insert("x-yun-api-version", HeaderValue::from_static("v1"));
    headers.insert("x-yun-app-channel", HeaderValue::from_static(MCLOUD_CHANNEL_SRC));
    headers.insert("x-yun-channel-source", HeaderValue::from_static(MCLOUD_CHANNEL_SRC));
    headers.insert(
        "x-yun-client-info",
        HeaderValue::from_static(CLIENT_INFO),
    );
    headers.insert("x-yun-module-type", HeaderValue::from_static("100"));
    headers.insert("x-yun-svc-type", HeaderValue::from_static("1"));
    headers.insert("Origin", HeaderValue::from_static("https://yun.139.com"));
    headers.insert("Referer", HeaderValue::from_static("https://yun.139.com/w/"));

    headers.insert(
        "Authorization",
        format!("Basic {}", config.authorization)
            .parse()
            .map_err(|e| ClientError::InvalidHeader(format!("{}", e)))?,
    );
    headers.insert(
        "mcloud-sign",
        format!("{},{},{}", ts, rand_str, sign)
            .parse()
            .map_err(|e| ClientError::InvalidHeader(format!("{}", e)))?,
    );
    headers.insert(
        "x-SvcType",
        svctype
            .parse()
            .map_err(|e| ClientError::InvalidHeader(format!("{}", e)))?,
    );

    let resp = client.post(url).headers(headers).json(&body).send().await?;

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
    let files = list_personal_files_with_client(config, parent_file_id, http_client).await?;
    Ok(files.iter().any(|f| f.name.as_deref() == Some(file_name)))
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
    let mut config = config.clone();
    let host = get_personal_cloud_host(&mut config).await?;
    let url = format!("{}/file/list", host);

    let body = serde_json::json!({
        "imageThumbnailStyleList": ["Small", "Large"],
        "orderBy": "updated_at",
        "orderDirection": "DESC",
        "pageInfo": {
            "pageCursor": "",
            "pageSize": 100
        },
        "parentFileId": parent_file_id
    });

    let resp: crate::models::PersonalListResp = personal_api_request_with_client(
        &config,
        &url,
        body,
        crate::client::StorageType::PersonalNew,
        http_client,
    )
    .await?;

    Ok(resp.data.map(|d| d.items).unwrap_or_default())
}

pub async fn get_family_download_link(
    config: &Config,
    content_id: &str,
    path: &str,
) -> Result<String, ClientError> {
    let client = crate::client::Client::new(config.clone());

    let body = serde_json::json!({
        "contentID": content_id,
        "path": path,
        "catalogType": 3,
        "cloudID": config.cloud_id,
        "cloudType": 1,
        "commonAccountInfo": {
            "account": config.account,
            "accountType": 1
        }
    });

    let resp: serde_json::Value = client
        .api_request_post(
            "https://yun.139.com/orchestration/familyCloud-rebuild/content/v1.0/getFileDownLoadURL",
            body,
        )
        .await?;

    let url = resp
        .pointer("/data/downloadURL")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    Ok(url)
}

pub async fn get_group_download_link(
    config: &Config,
    content_id: &str,
    path: &str,
) -> Result<String, ClientError> {
    let client = crate::client::Client::new(config.clone());

    let body = serde_json::json!({
        "contentID": content_id,
        "groupID": config.cloud_id,
        "path": path,
        "catalogType": 3,
        "cloudID": config.cloud_id,
        "cloudType": 1,
        "commonAccountInfo": {
            "account": config.account,
            "accountType": 1
        }
    });

    let resp: serde_json::Value = client.api_request_post(
        "https://yun.139.com/orchestration/group-rebuild/groupManage/v1.0/getGroupFileDownLoadURL",
        body
    ).await?;

    let url = resp
        .pointer("/data/downloadURL")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    Ok(url)
}

pub async fn get_family_root_path(config: &Config) -> Result<String, ClientError> {
    let client = crate::client::Client::new(config.clone());

    let body = serde_json::json!({
        "catalogID": "",
        "catalogType": 3,
        "cloudID": config.cloud_id,
        "cloudType": 1,
        "commonAccountInfo": {
            "account": config.account,
            "accountType": 1
        },
        "contentSortType": 0,
        "pageInfo": {
            "pageNum": 1,
            "pageSize": 1
        },
        "sortDirection": 1
    });

    let resp: serde_json::Value = client
        .api_request_post(
            "https://yun.139.com/orchestration/familyCloud-rebuild/content/v1.2/queryContentList",
            body,
        )
        .await?;

    let path = resp
        .pointer("/data/path")
        .and_then(|v| v.as_str())
        .map(|s| {
            let s = s.trim_start_matches("root:/");
            let s = s.trim_start_matches("root:");
            s.to_string()
        })
        .unwrap_or_default();

    if path.is_empty()
        && let Some(catalog_list) = resp
            .pointer("/data/cloudCatalogList")
            .and_then(|v| v.as_array())
            && let Some(first) = catalog_list.first()
            && let Some(p) = first.get("path").and_then(|v| v.as_str())
    {
        let p = p.trim_start_matches("root:/");
        let p = p.trim_start_matches("root:");
        return Ok(p.to_string());
    }

    Ok(path)
}

pub async fn get_group_root_by_cloud_id(config: &Config) -> Result<String, ClientError> {
    let client = crate::client::Client::new(config.clone());

    let body = serde_json::json!({
        "groupID": config.cloud_id,
        "commonAccountInfo": {
            "account": config.account,
            "accountType": 1
        },
        "pageInfo": {
            "pageNum": 1,
            "pageSize": 1
        }
    });

    let resp: serde_json::Value = client
        .api_request_post(
            "https://yun.139.com/orchestration/group-rebuild/catalog/v1.0/queryGroupContentList",
            body,
        )
        .await?;

    if let Some(parent_catalog_id) = resp
        .pointer("/data/getGroupContentResult/parentCatalogID")
        .and_then(|v| v.as_str())
        && !parent_catalog_id.is_empty()
    {
        return Ok(parent_catalog_id.to_string());
    }

    if let Some(catalog_list) = resp
        .pointer("/data/getGroupContentResult/catalogList")
        .and_then(|v| v.as_array())
        && let Some(first) = catalog_list.first()
        && let Some(p) = first.get("path").and_then(|v| v.as_str())
    {
        return Ok(p.to_string());
    }

    Err(ClientError::Other(
        "Failed to get group root path".to_string(),
    ))
}

pub async fn get_personal_download_link(
    config: &Config,
    file_id: &str,
) -> Result<String, ClientError> {
    let mut config = config.clone();
    let host = get_personal_cloud_host(&mut config).await?;
    let url = format!("{}/file/getDownloadUrl", host);

    let body = serde_json::json!({
        "fileId": file_id
    });

    let resp: serde_json::Value =
        personal_api_request(&config, &url, body, crate::client::StorageType::PersonalNew).await?;

    let cdn_url = resp
        .pointer("/data/cdnUrl")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if !cdn_url.is_empty() {
        return Ok(cdn_url);
    }

    let url = resp
        .pointer("/data/url")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    Ok(url)
}
