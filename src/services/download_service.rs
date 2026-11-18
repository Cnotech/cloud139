use crate::client::ClientError;
use crate::config::Config;
use crate::domain::StorageType;
use crate::models::DownloadUrlResp;
use anyhow::Result;
use futures_util::StreamExt;
use indicatif::ProgressBar;
use sha2::{Digest, Sha256};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

/// 下载文件服务
pub async fn download(
    config: &Config,
    remote_path: &str,
    local_path: &str,
    pb: Option<ProgressBar>,
) -> Result<()> {
    download_with_size(config, remote_path, local_path, pb, None).await
}

async fn download_with_size(
    config: &Config,
    remote_path: &str,
    local_path: &str,
    pb: Option<ProgressBar>,
    listed_size: Option<i64>,
) -> Result<()> {
    let normalized = super::directory_service::normalize(remote_path)?;
    let remote_path = normalized.as_str();
    let (file, size) = super::directory_service::resolve_with_size(config, remote_path).await?;
    if file.folder {
        return Err(ClientError::UnsupportedDownloadDirectory.into());
    }
    download_personal(
        config,
        remote_path,
        &file.id,
        local_path,
        pb,
        listed_size.or(size),
    )
    .await?;
    Ok(())
}

/// 下载个人云文件
async fn download_personal(
    config: &Config,
    remote_path: &str,
    file_id: &str,
    local_path: &str,
    pb: Option<ProgressBar>,
    listed_size: Option<i64>,
) -> Result<(), ClientError> {
    let mut config = config.clone();
    let host = crate::client::api::get_personal_cloud_host(&mut config).await?;

    let (download_url, reported_name) = personal_download_link(&config, &host, file_id).await?;

    let local_path_obj = Path::new(local_path);
    if local_path_obj.is_dir() {
        let file_name = reported_name.unwrap_or_else(|| {
            remote_path
                .trim_start_matches('/')
                .rsplit('/')
                .next()
                .unwrap_or(remote_path)
                .to_string()
        });
        super::directory_service::validate_name(&file_name)?;
        let file_path = local_path_obj.join(&file_name);
        download_file(&download_url, &file_path, pb, listed_size).await?;
    } else {
        download_file(&download_url, local_path_obj, pb, listed_size).await?;
    }

    Ok(())
}

async fn personal_download_link(
    config: &Config,
    host: &str,
    file_id: &str,
) -> Result<(String, Option<String>), ClientError> {
    let resp: DownloadUrlResp = crate::client::api::personal_api_request(
        config,
        &format!("{host}/file/getDownloadUrl"),
        serde_json::json!({ "fileId": file_id }),
        StorageType::PersonalNew,
    )
    .await?;
    if !resp.base.success {
        return Err(ClientError::Api(format!(
            "获取下载链接失败: {}",
            resp.base.message.as_deref().unwrap_or("未知错误")
        )));
    }
    let download_url = resp
        .data
        .cdn_url
        .or(resp.data.url)
        .filter(|url| !url.is_empty())
        .ok_or_else(|| ClientError::Api("获取下载链接失败: URL为空".into()))?;
    Ok((download_url, resp.data.file_name))
}

pub(super) async fn download_scanned_personal(
    config: &Config,
    host: &str,
    file_id: &str,
    local_path: &Path,
    pb: Option<ProgressBar>,
    size: Option<u64>,
) -> Result<(), ClientError> {
    let (url, _) = personal_download_link(config, host, file_id).await?;
    download_file(
        &url,
        local_path,
        pb,
        size.and_then(|size| i64::try_from(size).ok()),
    )
    .await
}

pub(super) async fn personal_content_sha256(
    config: &Config,
    host: &str,
    file_id: &str,
    expected_size: Option<u64>,
) -> Result<String, ClientError> {
    let (url, _) = personal_download_link(config, host, file_id).await?;
    let client = reqwest::Client::builder()
        .read_timeout(std::time::Duration::from_secs(30))
        .build()?;
    let response = client
        .get(url)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(reqwest::Error::without_url)?;
    let mut body = response.bytes_stream();
    let mut hasher = Sha256::new();
    let mut bytes = 0;
    while let Some(chunk) = body.next().await {
        let chunk = chunk.map_err(reqwest::Error::without_url)?;
        bytes += chunk.len() as u64;
        if expected_size.is_some_and(|size| bytes > size) {
            return Err(ClientError::Api(
                "读取校验内容时，远端文件大于预期大小".into(),
            ));
        }
        hasher.update(&chunk);
    }
    if expected_size.is_some_and(|size| bytes != size) {
        return Err(ClientError::Api(
            "读取校验内容时，远端文件大小与列表不一致".into(),
        ));
    }
    Ok(hex::encode(hasher.finalize()))
}

/// 下载文件到本地
async fn download_file(
    url: &str,
    local_path: &Path,
    pb: Option<ProgressBar>,
    listed_size: Option<i64>,
) -> Result<(), ClientError> {
    super::directory_service::safe_local(local_path)?;
    let existing = local_file_metadata(local_path)?;
    // 重试会复用同一个进度条（clone 共享状态）；每次下载都从头写文件，进度也要归零。
    if let Some(pb) = &pb {
        pb.set_position(0);
        pb.reset_eta();
    }

    let client = crate::client::api::HttpClientWrapper::new();
    // Reject HTTP error pages before creating or truncating the destination.
    let response = client
        .client
        .get(url)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(reqwest::Error::without_url)?;

    let parent = super::directory_service::local_parent_directory(local_path);
    std::fs::create_dir_all(parent)?;

    let expected_size = listed_size
        .and_then(|size| u64::try_from(size).ok())
        .or_else(|| response.content_length());
    if let (Some(pb), Some(total)) = (&pb, expected_size) {
        pb.set_length(total);
    }

    let mut file = {
        let mut staged = tempfile::Builder::new();
        if existing.is_none() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                staged.permissions(std::fs::Permissions::from_mode(0o666));
            }
        }
        staged.tempfile_in(parent)?
    };
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(reqwest::Error::without_url)?;
        file.write_all(&chunk)?;
        if let Some(pb) = &pb {
            pb.inc(chunk.len() as u64);
        }
    }
    file.flush()?;
    if let Some(expected) = expected_size {
        let actual = file.as_file().metadata()?.len();
        if actual != expected {
            return Err(ClientError::Api(format!(
                "下载大小不匹配：预期 {expected} 字节，实际 {actual} 字节"
            )));
        }
    }
    super::directory_service::safe_local(local_path)?;
    if let Some(metadata) = local_file_metadata(local_path)? {
        file.as_file().set_permissions(metadata.permissions())?;
    }
    file.persist(local_path).map_err(|error| error.error)?;

    if let Some(pb) = pb {
        pb.finish_and_clear();
    }

    Ok(())
}

fn local_file_metadata(path: &Path) -> Result<Option<std::fs::Metadata>, ClientError> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() && !metadata.permissions().readonly() => {
            Ok(Some(metadata))
        }
        Ok(_) => Err(super::directory_service::invalid(format!(
            "本地目标不是可写的普通文件: {}",
            path.display()
        ))),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

/// Recursive download retains empty directories and rejects unsafe names and local symlinks.
pub async fn download_recursive(
    config: &Config,
    remote: &str,
    local: &str,
    recursive: bool,
) -> Result<()> {
    use super::directory_service as dirs;
    config.require_personal()?;
    let remote = dirs::normalize(remote)?;
    dirs::safe_local(Path::new(local))?;
    let (entry, size) = dirs::resolve_with_size(config, &remote).await?;
    if !entry.folder {
        return Ok(download_personal(config, &remote, &entry.id, local, None, size).await?);
    }
    if !recursive {
        return Err(ClientError::UnsupportedDownloadDirectory.into());
    }
    let entries = dirs::tree(config, &remote).await?;
    let destination_root = Path::new(local);
    let mut plan = Vec::with_capacity(entries.len());
    // Check all destinations before writing any file.
    for entry in entries {
        let rel = entry
            .path
            .strip_prefix(remote.trim_end_matches('/'))
            .unwrap()
            .trim_start_matches('/');
        let rel = PathBuf::from(rel);
        let dest = destination_root.join(&rel);
        dirs::safe_local(&dest)?;
        if dest.exists() {
            if dest.is_dir() != entry.folder {
                return Err(dirs::invalid("本地文件与目录冲突").into());
            }
            if !entry.folder {
                local_file_metadata(&dest)?;
            }
        }
        plan.push((entry, dest));
    }
    if !plan.is_empty() {
        let paths = plan.iter().map(|(entry, destination)| {
            (
                destination
                    .strip_prefix(destination_root)
                    .expect("planned descendant"),
                entry.folder,
            )
        });
        dirs::check_destination_collisions(destination_root, paths)?;
    }
    std::fs::create_dir_all(destination_root)?;
    for (entry, dest) in plan {
        dirs::safe_local(&dest)?;
        if entry.folder {
            std::fs::create_dir_all(&dest)?;
        } else {
            download_with_size(
                config,
                &entry.path,
                dest.to_str().ok_or(ClientError::InvalidFilePath)?,
                None,
                entry.size,
            )
            .await
            .map_err(|e| anyhow::anyhow!("下载 {} 失败；此前条目可能已完成: {}", entry.path, e))?;
        }
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

        download_file(&url, &dir.path().join("a"), Some(pb.clone()), None)
            .await
            .expect("first download");
        assert_eq!(pb.position(), 10);

        download_file(&url, &dir.path().join("b"), Some(pb.clone()), None)
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

        let target = dir.path().join("a");
        std::fs::write(&target, b"existing").unwrap();
        let first = download_file(&url, &target, Some(pb.clone()), None).await;
        assert!(first.is_err(), "半包应当报错");
        assert_eq!(std::fs::read(&target).unwrap(), b"existing");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
        let after_first = pb.position();

        download_file(&url, &target, Some(pb.clone()), None)
            .await
            .expect("retry download");
        assert_eq!(std::fs::read(&target).unwrap(), b"0123456789");
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

        let result = download_file(&url, &dir.path().join("out"), Some(pb.clone()), None).await;
        assert!(result.is_err(), "incomplete body must fail");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
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
            let err = download_file(&server.url("/download"), path, None, None)
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
