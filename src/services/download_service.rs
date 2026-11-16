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

    // 父目录可能超过一页（>100 个子项），按页找、命中即停，避免后续页的失败
    // 拖垮本来能判断的目录检查。
    let child = crate::client::api::find_personal_child(
        &config,
        &host,
        &parent_id,
        file_name,
        &crate::client::api::HttpClientWrapper::new(),
    )
    .await?;

    if let Some(item) = child
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
    // 重试会复用同一个进度条（clone 共享状态）；每次下载都从头写文件，进度也要归零。
    if let Some(pb) = &pb {
        pb.set_position(0);
        pb.reset_eta();
    }

    let client = reqwest::Client::new();
    // Reject HTTP error pages before creating or truncating the destination.
    let response = client.get(url).send().await?.error_for_status()?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;

    /// 第一个连接只发一半的 body（触发不完整响应），其余连接完整发送。
    fn spawn_server(partial_first: bool, body: &'static [u8]) -> (String, Arc<AtomicUsize>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let hits = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&hits);
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut buf = [0u8; 4096];
                let _ = stream.read(&mut buf);
                let n = counter.fetch_add(1, Ordering::SeqCst);
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(header.as_bytes());
                if partial_first && n == 0 {
                    let _ = stream.write_all(&body[..body.len() / 2]);
                } else {
                    let _ = stream.write_all(body);
                }
                let _ = stream.flush();
            }
        });
        (format!("http://{addr}"), hits)
    }

    /// 同一个进度条复用（重试）时，进度必须从 0 重新累加，不能叠加。
    #[tokio::test]
    async fn download_restarts_progress_on_retry() {
        let (url, _hits) = spawn_server(false, b"0123456789");
        let dir = tempfile::tempdir().expect("tmp");
        let pb = ProgressBar::new(0);

        download_file(&url, &dir.path().join("a"), Some(pb.clone()))
            .await
            .expect("first download");
        assert_eq!(pb.position(), 10);

        download_file(&url, &dir.path().join("b"), Some(pb.clone()))
            .await
            .expect("second download");
        assert_eq!(pb.position(), 10, "第二次应从头累加，不是 20");
    }

    /// 中途断流后重试：进度先归零，最终不超过文件大小。
    #[tokio::test]
    async fn partial_body_then_retry_keeps_position_within_file_size() {
        let (url, hits) = spawn_server(true, b"0123456789");
        let dir = tempfile::tempdir().expect("tmp");
        let pb = ProgressBar::new(0);

        let first = download_file(&url, &dir.path().join("a"), Some(pb.clone())).await;
        assert!(first.is_err(), "半包应当报错");
        let after_first = pb.position();

        download_file(&url, &dir.path().join("a"), Some(pb.clone()))
            .await
            .expect("retry download");
        assert_eq!(
            pb.position(),
            10,
            "重试前归零，结束后等于文件大小（首次已下载 {after_first}）"
        );
        assert_eq!(hits.load(Ordering::SeqCst), 2);
    }

    /// Assert before successful completion can normalize the position to the length.
    #[tokio::test]
    async fn failed_download_resets_stale_progress_before_completion() {
        let (url, _) = spawn_server(true, b"0123456789");
        let dir = tempfile::tempdir().expect("tmp");
        let pb = ProgressBar::new(10);
        pb.set_position(7);

        let result = download_file(&url, &dir.path().join("out"), Some(pb.clone())).await;
        assert!(result.is_err(), "incomplete body must fail");
        assert!(
            pb.position() <= 5,
            "stale progress must be cleared before streaming: {}",
            pb.position()
        );
    }

    #[tokio::test]
    async fn http_error_does_not_create_or_truncate_destination() {
        use httpmock::prelude::*;
        let server = MockServer::start();
        let error_page = server.mock(|when, then| {
            when.method(GET).path("/download");
            then.status(503).body("upstream unavailable");
        });
        let dir = tempfile::tempdir().expect("tmp");
        let existing = dir.path().join("existing");
        std::fs::write(&existing, b"keep original").expect("write original");
        let missing = dir.path().join("missing");

        for path in [&existing, &missing] {
            let err = download_file(&server.url("/download"), path, None)
                .await
                .expect_err("HTTP error must not be treated as a file");
            match err {
                ClientError::Http(err) => {
                    assert_eq!(err.status(), Some(reqwest::StatusCode::SERVICE_UNAVAILABLE))
                }
                other => panic!("unexpected error: {other}"),
            }
        }
        assert_eq!(
            std::fs::read(&existing).expect("original remains"),
            b"keep original"
        );
        assert!(!missing.exists(), "HTTP error must not create destination");
        error_page.assert_calls(2);
    }
}
