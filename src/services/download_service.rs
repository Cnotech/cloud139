use crate::client::ClientError;
use crate::client::endpoints::{family, group};
use crate::config::Config;
use crate::debug;
use crate::domain::StorageType;
use crate::models::DownloadUrlResp;
use anyhow::Result;
use indicatif::ProgressBar;
use std::path::Path;

/// 下载文件服务
pub async fn download(
    config: &Config,
    remote_path: &str,
    local_path: &str,
    pb: Option<ProgressBar>,
) -> Result<()> {
    let storage_type = config.storage_type();
    debug!(
        "download: remote={}, local={}, storage={}",
        remote_path,
        local_path,
        storage_type.as_str()
    );

    match storage_type {
        StorageType::PersonalNew => {
            let file_id = crate::client::api::get_file_id_by_path(config, remote_path).await?;
            if file_id.is_empty() {
                return Err(ClientError::InvalidFilePath.into());
            }
            debug!("download_personal: file_id={}", file_id);
            download_personal(config, remote_path, &file_id, local_path, pb).await?;
        }
        StorageType::Family => {
            download_family(config, remote_path, local_path, pb).await?;
        }
        StorageType::Group => {
            download_group(config, remote_path, local_path, pb).await?;
        }
    }

    Ok(())
}

/// 下载个人云文件
async fn download_personal(
    config: &Config,
    remote_path: &str,
    file_id: &str,
    local_path: &str,
    pb: Option<ProgressBar>,
) -> Result<(), ClientError> {
    let mut config = config.clone();
    let host = crate::client::api::get_personal_cloud_host(&mut config).await?;

    let parts: Vec<&str> = remote_path
        .trim_start_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();
    let file_name = parts.last().unwrap_or(&remote_path);
    let parent_id = if parts.len() > 1 {
        crate::client::api::get_file_id_by_path(&config, &parts[..parts.len() - 1].join("/"))
            .await?
    } else {
        "/".to_string()
    };

    let list_url = format!("{}/file/list", host);
    let list_body = serde_json::json!({
        "parentFileId": parent_id,
        "pageInfo": {
            "pageCursor": "",
            "pageSize": 100
        },
        "orderBy": "updated_at",
        "orderDirection": "DESC"
    });
    let list_resp: crate::models::PersonalListResp = crate::client::api::personal_api_request(
        &config,
        &list_url,
        list_body,
        StorageType::PersonalNew,
    )
    .await?;

    if let Some(items) = list_resp.data.map(|d| d.items)
        && let Some(item) = items
            .iter()
            .find(|item| item.name.as_deref() == Some(file_name))
        && (item.file_type.as_deref() == Some("1")
            || item.file_type.as_deref() == Some("folder")
            || item.file_type.as_deref() == Some("dir"))
    {
        return Err(ClientError::UnsupportedDownloadDirectory);
    }

    let url = format!("{}/file/getDownloadUrl", host);

    let body = serde_json::json!({
        "fileId": file_id,
    });

    let resp: DownloadUrlResp =
        crate::client::api::personal_api_request(&config, &url, body, StorageType::PersonalNew)
            .await?;

    if !resp.base.success {
        return Err(ClientError::Api(format!(
            "获取下载链接失败: {}",
            resp.base.message.as_deref().unwrap_or("未知错误")
        )));
    }

    let download_url = resp.data.cdn_url.or(resp.data.url).unwrap_or_default();
    if download_url.is_empty() {
        return Err(ClientError::Api("获取下载链接失败: URL为空".to_string()));
    }
    debug!(
        "download_personal: 获取下载链接, url_len={}",
        download_url.len()
    );

    let local_path_obj = Path::new(local_path);
    if local_path_obj.is_dir() {
        let file_name = resp.data.file_name.unwrap_or_else(|| {
            remote_path
                .trim_start_matches('/')
                .rsplit('/')
                .next()
                .unwrap_or(remote_path)
                .to_string()
        });
        let file_path = local_path_obj.join(&file_name);
        download_file(&download_url, &file_path, pb.clone()).await?;
    } else {
        download_file(&download_url, local_path_obj, pb).await?;
    }

    Ok(())
}

/// 下载文件到本地
async fn download_file(
    url: &str,
    local_path: &Path,
    pb: Option<ProgressBar>,
) -> Result<(), ClientError> {
    let client = reqwest::Client::new();
    let response = client.get(url).send().await?;

    if let Some(parent) = local_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let total_size = response.content_length();
    if let (Some(pb), Some(total)) = (&pb, total_size) {
        pb.set_length(total);
    }

    let mut file = std::fs::File::create(local_path)?;
    let mut stream = response.bytes_stream();

    use futures_util::StreamExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        use std::io::Write;
        file.write_all(&chunk)?;
        if let Some(pb) = &pb {
            pb.inc(chunk.len() as u64);
        }
    }

    if let Some(pb) = pb {
        pb.finish_and_clear();
    }

    Ok(())
}

/// 下载家庭云文件
async fn download_family(
    config: &Config,
    remote_path: &str,
    local_path: &str,
    pb: Option<ProgressBar>,
) -> Result<(), ClientError> {
    let parts: Vec<&str> = remote_path.trim_start_matches('/').split('/').collect();
    if parts.is_empty() {
        return Err(ClientError::InvalidFilePath);
    }

    let file_name = parts.last().unwrap();
    let parent_path = if parts.len() > 1 {
        parts[..parts.len() - 1].join("/")
    } else {
        config
            .root_folder_id
            .clone()
            .unwrap_or_else(|| "0".to_string())
    };

    let url = family::orchestration::QUERY_CONTENT_LIST;

    let body = serde_json::json!({
        "catalogID": parent_path,
        "sortType": 1,
        "pageNumber": 1,
        "pageSize": 100,
        "cloudID": config.cloud_id,
        "cloudType": 1,
        "commonAccountInfo": {
            "account": config.account,
            "accountType": 1
        }
    });

    let client = crate::client::Client::new(config.clone());
    let resp: serde_json::Value = client.api_request_post(url, body).await?;

    let mut found_id: Option<String> = None;
    let mut found_path: Option<String> = None;

    if let Some(catalog_list) = resp
        .pointer("/data/cloudCatalogList")
        .and_then(|v| v.as_array())
    {
        for cat in catalog_list {
            if cat.get("catalogName").and_then(|v| v.as_str()) == Some(file_name) {
                found_id = cat
                    .get("catalogID")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                break;
            }
        }
    }

    if found_id.is_none()
        && let Some(content_list) = resp
            .pointer("/data/cloudContentList")
            .and_then(|v| v.as_array())
    {
        for content in content_list {
            if content.get("contentName").and_then(|v| v.as_str()) == Some(file_name) {
                found_id = content
                    .get("contentID")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                found_path = resp
                    .pointer("/data/path")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                break;
            }
        }
    }

    let content_id = match found_id {
        Some(id) => id,
        None => {
            return Err(ClientError::FileNotFound);
        }
    };

    if let Some(catalog_list) = resp
        .pointer("/data/cloudCatalogList")
        .and_then(|v| v.as_array())
    {
        for cat in catalog_list {
            if cat.get("catalogName").and_then(|v| v.as_str()) == Some(file_name) {
                return Err(ClientError::UnsupportedDownloadDirectory);
            }
        }
    }

    let path = found_path.unwrap_or_else(|| parent_path.clone());

    let download_url =
        crate::client::api::get_family_download_link(config, &content_id, &path).await?;

    if download_url.is_empty() {
        return Err(ClientError::Api("获取下载链接失败: URL为空".to_string()));
    }

    let local_path_obj = std::path::Path::new(local_path);
    if local_path_obj.is_dir() {
        let file_path = local_path_obj.join(file_name);
        download_file(&download_url, &file_path, pb.clone()).await?;
    } else {
        download_file(&download_url, local_path_obj, pb).await?;
    }

    Ok(())
}

/// 下载群组云文件
async fn download_group(
    config: &Config,
    remote_path: &str,
    local_path: &str,
    pb: Option<ProgressBar>,
) -> Result<(), ClientError> {
    let parts: Vec<&str> = remote_path.trim_start_matches('/').split('/').collect();
    if parts.is_empty() {
        return Err(ClientError::InvalidFilePath);
    }

    let file_name = parts.last().unwrap();
    let parent_path = if parts.len() > 1 {
        parts[..parts.len() - 1].join("/")
    } else {
        "0".to_string()
    };

    let url = group::orchestration::QUERY_GROUP_CONTENT_LIST;

    let body = serde_json::json!({
        "groupID": config.cloud_id,
        "catalogID": parent_path,
        "contentSortType": 0,
        "sortDirection": 1,
        "startNumber": 1,
        "endNumber": 100,
        "path": if parent_path == "0" { "root:".to_string() } else { format!("root:/{}", parent_path) }
    });

    let client = crate::client::Client::new(config.clone());
    let resp: serde_json::Value = client.api_request_post(url, body).await?;

    let mut found_id: Option<String> = None;
    let mut found_path: Option<String> = None;

    if let Some(catalog_list) = resp
        .pointer("/data/getGroupContentResult/catalogList")
        .and_then(|v| v.as_array())
    {
        for cat in catalog_list {
            if cat.get("catalogName").and_then(|v| v.as_str()) == Some(file_name) {
                found_id = cat
                    .get("catalogID")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                found_path = cat
                    .get("path")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                break;
            }
        }
    }

    if found_id.is_none()
        && let Some(content_list) = resp
            .pointer("/data/getGroupContentResult/contentList")
            .and_then(|v| v.as_array())
    {
        for content in content_list {
            if content.get("contentName").and_then(|v| v.as_str()) == Some(file_name) {
                found_id = content
                    .get("contentID")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                found_path = resp
                    .pointer("/data/getGroupContentResult/parentCatalogID")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                break;
            }
        }
    }

    let content_id = match found_id {
        Some(id) => id,
        None => {
            return Err(ClientError::FileNotFound);
        }
    };

    if let Some(catalog_list) = resp
        .pointer("/data/getGroupContentResult/catalogList")
        .and_then(|v| v.as_array())
    {
        for cat in catalog_list {
            if cat.get("catalogName").and_then(|v| v.as_str()) == Some(file_name) {
                return Err(ClientError::UnsupportedDownloadDirectory);
            }
        }
    }

    let path = found_path.unwrap_or_else(|| format!("root:/{}", parent_path));

    let download_url =
        crate::client::api::get_group_download_link(config, &content_id, &path).await?;

    if download_url.is_empty() {
        return Err(ClientError::Api("获取下载链接失败: URL为空".to_string()));
    }

    let local_path_obj = std::path::Path::new(local_path);
    if local_path_obj.is_dir() {
        let file_path = local_path_obj.join(file_name);
        download_file(&download_url, &file_path, pb.clone()).await?;
    } else {
        download_file(&download_url, local_path_obj, pb).await?;
    }

    Ok(())
}
