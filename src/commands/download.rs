use clap::Parser;
use std::path::Path;
use crate::client::{ClientError, StorageType};
use crate::models::DownloadUrlResp;
use crate::{info, success, error, step};

#[derive(Parser, Debug)]
pub struct DownloadArgs {
    #[arg(help = "远程文件路径")]
    pub remote_path: String,

    #[arg(help = "本地保存路径（默认保存到当前目录的同名文件）")]
    pub local_path: Option<String>,
}

pub async fn execute(args: DownloadArgs) -> Result<(), ClientError> {
    let config = crate::config::Config::load().map_err(ClientError::Config)?;
    let storage_type = config.storage_type();

    let remote_path = &args.remote_path;
    let local_path = match &args.local_path {
        Some(path) if !path.is_empty() => path.clone(),
        _ => {
            let parts: Vec<&str> = remote_path.trim_start_matches('/').rsplit('/').collect();
            let file_name = parts.first().copied().unwrap_or_else(|| remote_path.as_str());
            if file_name.is_empty() || file_name == remote_path {
                "download".to_string()
            } else {
                file_name.to_string()
            }
        }
    };

    match storage_type {
        StorageType::PersonalNew => {
            let file_id = crate::client::api::get_file_id_by_path(&config, remote_path).await?;
            if file_id.is_empty() {
                error!("无效的文件路径");
                return Ok(());
            }
            download_personal(&config, remote_path, &file_id, &local_path).await?;
        }
        StorageType::Family => {
            download_family(&config, remote_path, &local_path).await?;
        }
        StorageType::Group => {
            download_group(&config, remote_path, &local_path).await?;
        }
    }

    Ok(())
}

async fn download_personal(
    config: &crate::config::Config,
    remote_path: &str,
    file_id: &str,
    local_path: &str,
) -> Result<(), ClientError> {
    let mut config = config.clone();
    let host = crate::client::api::get_personal_cloud_host(&mut config).await?;
    let url = format!("{}/file/getDownloadUrl", host);

    let body = serde_json::json!({
        "fileId": file_id,
    });

    let resp: DownloadUrlResp = crate::client::api::personal_api_request(&config, &url, body, StorageType::PersonalNew).await?;

    if !resp.base.success {
        return Err(ClientError::Api(format!("获取下载链接失败: {}", resp.base.message.as_deref().unwrap_or("未知错误"))));
    }

    let download_url = resp.data.cdn_url.or(resp.data.url).unwrap_or_default();
    if download_url.is_empty() {
        return Err(ClientError::Api("获取下载链接失败: URL为空".to_string()));
    }

    info!("下载链接: {}", download_url);

    let local_path_obj = Path::new(local_path);
    if local_path_obj.is_dir() {
        let file_name = resp.data.file_name
            .unwrap_or_else(|| {
                let parts: Vec<&str> = remote_path.trim_start_matches('/').rsplit('/').collect();
                parts.first().copied()
                    .unwrap_or_else(|| remote_path)
                    .to_string()
            });
        let file_path = local_path_obj.join(&file_name);
        download_file(&download_url, &file_path).await?;
    } else {
        download_file(&download_url, local_path_obj).await?;
    }

    Ok(())
}

async fn download_file(url: &str, local_path: &Path) -> Result<(), ClientError> {
    step!("开始下载到: {:?}", local_path);

    let client = reqwest::Client::new();
    let response = client.get(url).send().await?;

    if let Some(parent) = local_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let total_size = response.content_length();
    info!("文件大小: {} bytes", total_size.unwrap_or(0));

    let mut file = std::fs::File::create(local_path)?;
    let mut downloaded: u64 = 0;
    let mut stream = response.bytes_stream();

    use futures_util::StreamExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        use std::io::Write;
        file.write_all(&chunk)?;
        downloaded += chunk.len() as u64;
        if let Some(total) = total_size {
            print!("\r下载进度: {}/{} ({:.1}%)", downloaded, total, downloaded as f64 / total as f64 * 100.0);
        }
    }

    success!("下载完成!");
    Ok(())
}

async fn download_family(
    config: &crate::config::Config,
    remote_path: &str,
    local_path: &str,
) -> Result<(), ClientError> {
    let parts: Vec<&str> = remote_path.trim_start_matches('/').split('/').collect();
    if parts.is_empty() {
        error!("无效的文件路径");
        return Ok(());
    }

    let file_name = parts.last().unwrap();
    let parent_path = if parts.len() > 1 {
        parts[..parts.len()-1].join("/")
    } else {
        config.root_folder_id.clone().unwrap_or_else(|| "0".to_string())
    };

    let url = "https://yun.139.com/orchestration/familyCloud-rebuild/content/v1.2/queryContentList";
    
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

    if let Some(catalog_list) = resp.pointer("/data/cloudCatalogList").and_then(|v| v.as_array()) {
        for cat in catalog_list {
            if cat.get("catalogName").and_then(|v| v.as_str()) == Some(file_name) {
                found_id = cat.get("catalogID").and_then(|v| v.as_str()).map(|s| s.to_string());
                break;
            }
        }
    }

    if found_id.is_none() {
        if let Some(content_list) = resp.pointer("/data/cloudContentList").and_then(|v| v.as_array()) {
            for content in content_list {
                if content.get("contentName").and_then(|v| v.as_str()) == Some(file_name) {
                    found_id = content.get("contentID").and_then(|v| v.as_str()).map(|s| s.to_string());
                    found_path = resp.pointer("/data/path").and_then(|v| v.as_str()).map(|s| s.to_string());
                    break;
                }
            }
        }
    }

    let content_id = match found_id {
        Some(id) => id,
        None => {
            error!("文件不存在");
            return Ok(());
        }
    };

    let path = found_path.unwrap_or_else(|| parent_path.clone());

    let download_url = crate::client::api::get_family_download_link(config, &content_id, &path).await?;
    
    if download_url.is_empty() {
        return Err(ClientError::Api("获取下载链接失败: URL为空".to_string()));
    }

    info!("下载链接: {}", download_url);

    let local_path_obj = std::path::Path::new(local_path);
    if local_path_obj.is_dir() {
        let file_path = local_path_obj.join(file_name);
        download_file(&download_url, &file_path).await?;
    } else {
        download_file(&download_url, local_path_obj).await?;
    }

    Ok(())
}

async fn download_group(
    config: &crate::config::Config,
    remote_path: &str,
    local_path: &str,
) -> Result<(), ClientError> {
    let parts: Vec<&str> = remote_path.trim_start_matches('/').split('/').collect();
    if parts.is_empty() {
        error!("无效的文件路径");
        return Ok(());
    }

    let file_name = parts.last().unwrap();
    let parent_path = if parts.len() > 1 {
        parts[..parts.len()-1].join("/")
    } else {
        "0".to_string()
    };

    let url = "https://yun.139.com/orchestration/group-rebuild/content/v1.0/queryGroupContentList";
    
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

    if let Some(catalog_list) = resp.pointer("/data/getGroupContentResult/catalogList").and_then(|v| v.as_array()) {
        for cat in catalog_list {
            if cat.get("catalogName").and_then(|v| v.as_str()) == Some(file_name) {
                found_id = cat.get("catalogID").and_then(|v| v.as_str()).map(|s| s.to_string());
                found_path = cat.get("path").and_then(|v| v.as_str()).map(|s| s.to_string());
                break;
            }
        }
    }

    if found_id.is_none() {
        if let Some(content_list) = resp.pointer("/data/getGroupContentResult/contentList").and_then(|v| v.as_array()) {
            for content in content_list {
                if content.get("contentName").and_then(|v| v.as_str()) == Some(file_name) {
                    found_id = content.get("contentID").and_then(|v| v.as_str()).map(|s| s.to_string());
                    found_path = resp.pointer("/data/getGroupContentResult/parentCatalogID").and_then(|v| v.as_str()).map(|s| s.to_string());
                    break;
                }
            }
        }
    }

    let content_id = match found_id {
        Some(id) => id,
        None => {
            error!("文件不存在");
            return Ok(());
        }
    };

    let path = found_path.unwrap_or_else(|| format!("root:/{}", parent_path));

    let download_url = crate::client::api::get_group_download_link(config, &content_id, &path).await?;
    
    if download_url.is_empty() {
        return Err(ClientError::Api("获取下载链接失败: URL为空".to_string()));
    }

    info!("下载链接: {}", download_url);

    let local_path_obj = std::path::Path::new(local_path);
    if local_path_obj.is_dir() {
        let file_path = local_path_obj.join(file_name);
        download_file(&download_url, &file_path).await?;
    } else {
        download_file(&download_url, local_path_obj).await?;
    }

    Ok(())
}
