use crate::client::api::personal::transfer::DownloadLink;
use crate::client::api::personal::{ListingRetry, PersonalClient};
use crate::client::http::HttpClientWrapper;
use crate::config::{Config, PersonalConfig};
use crate::error::{ClientError, invalid};
use crate::progress::Progress;
use crate::utils::path::{check_destination_collisions, local_parent_directory, safe_local};
use crate::utils::remote_path;
use anyhow::Result;
use sha2::{Digest, Sha256};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

async fn download_personal(
    personal: &PersonalClient<'_>,
    remote_path: &str,
    file_id: &str,
    local_path: &str,
    pb: Option<Progress>,
    listed_size: Option<i64>,
) -> Result<(), ClientError> {
    let mut link = personal.download_link(file_id).await?;

    let local_path_obj = Path::new(local_path);
    if local_path_obj.is_dir() {
        let file_name = link.name.take().unwrap_or_else(|| {
            remote_path
                .trim_start_matches('/')
                .rsplit('/')
                .next()
                .unwrap_or(remote_path)
                .to_string()
        });
        remote_path::validate_name(&file_name)?;
        let file_path = local_path_obj.join(&file_name);
        write_download(link, &file_path, pb, listed_size).await?;
    } else {
        write_download(link, local_path_obj, pb, listed_size).await?;
    }

    Ok(())
}

pub(super) async fn download_scanned_personal(
    config: PersonalConfig<&Config>,
    host: &str,
    file_id: &str,
    local_path: &Path,
    pb: Option<Progress>,
    size: Option<u64>,
) -> Result<(), ClientError> {
    let client = HttpClientWrapper::new();
    let personal = PersonalClient::with_host(config, host, &client, ListingRetry::None);
    download_to_path(
        &personal,
        file_id,
        local_path,
        pb,
        size.and_then(|size| i64::try_from(size).ok()),
    )
    .await
}

async fn download_to_path(
    personal: &PersonalClient<'_>,
    file_id: &str,
    destination: &Path,
    pb: Option<Progress>,
    size: Option<i64>,
) -> Result<(), ClientError> {
    let link = personal.download_link(file_id).await?;
    write_download(link, destination, pb, size).await
}

pub(super) async fn personal_content_sha256(
    config: PersonalConfig<&Config>,
    host: &str,
    file_id: &str,
    expected_size: Option<u64>,
) -> Result<String, ClientError> {
    let client = HttpClientWrapper::new();
    let personal = PersonalClient::with_host(config, host, &client, ListingRetry::None);
    let mut body = personal.download_link(file_id).await?.open().await?;
    let mut hasher = Sha256::new();
    let mut bytes = 0;
    while let Some(chunk) = body.next_chunk().await? {
        let chunk = chunk.as_ref();
        bytes += chunk.len() as u64;
        if expected_size.is_some_and(|size| bytes > size) {
            return Err(ClientError::Api(
                "读取校验内容时，远端文件大于预期大小".into(),
            ));
        }
        hasher.update(chunk);
    }
    if expected_size.is_some_and(|size| bytes != size) {
        return Err(ClientError::Api(
            "读取校验内容时，远端文件大小与列表不一致".into(),
        ));
    }
    Ok(hex::encode(hasher.finalize()))
}

/// 下载文件到本地
async fn write_download(
    link: DownloadLink<'_>,
    local_path: &Path,
    pb: Option<Progress>,
    listed_size: Option<i64>,
) -> Result<(), ClientError> {
    safe_local(local_path)?;
    let existing = local_file_metadata(local_path)?;
    // 重试会复用同一个进度条（clone 共享状态）；每次下载都从头写文件，进度也要归零。
    if let Some(pb) = &pb {
        pb.reset();
    }

    let mut response = link.open().await?;

    let parent = local_parent_directory(local_path);
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
    while let Some(chunk) = response.next_chunk().await? {
        let chunk = chunk.as_ref();
        file.write_all(chunk)?;
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
    safe_local(local_path)?;
    if let Some(metadata) = local_file_metadata(local_path)? {
        file.as_file().set_permissions(metadata.permissions())?;
    }
    file.persist(local_path).map_err(|error| error.error)?;

    if let Some(pb) = pb {
        pb.finish();
    }

    Ok(())
}

fn local_file_metadata(path: &Path) -> Result<Option<std::fs::Metadata>, ClientError> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() && !metadata.permissions().readonly() => {
            Ok(Some(metadata))
        }
        Ok(_) => Err(invalid(format!(
            "本地目标不是可写的普通文件: {}",
            path.display()
        ))),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

pub async fn download_path(
    config: &Config,
    remote: &str,
    local: &Option<String>,
    recursive: bool,
) -> Result<()> {
    let config = PersonalConfig::new(config)?;
    let remote = remote_path::normalize(remote)?;
    let local = crate::utils::path::resolve_local_path(&remote, local);
    crate::debug!("准备下载文件 remote={} local={}", remote, local);
    crate::step!("开始下载到: {:?}", local);
    download_normalized(config, remote, &local, None, recursive).await
}

/// 递归下载保留空目录，并拒绝不安全的名称和本地符号链接。
pub async fn download(
    config: &Config,
    remote: &str,
    local: &str,
    pb: Option<Progress>,
    recursive: bool,
) -> Result<()> {
    download_remote_path(PersonalConfig::new(config)?, remote, local, pb, recursive).await
}

pub(super) async fn download_remote_path(
    config: PersonalConfig<&Config>,
    remote: &str,
    local: &str,
    pb: Option<Progress>,
    recursive: bool,
) -> Result<()> {
    let remote = remote_path::normalize(remote)?;
    download_normalized(config, remote, local, pb, recursive).await
}

async fn download_normalized(
    config: PersonalConfig<&Config>,
    remote: String,
    local: &str,
    pb: Option<Progress>,
    recursive: bool,
) -> Result<()> {
    use super::directory as dirs;
    safe_local(Path::new(local))?;
    if remote == "/" && !recursive {
        return Err(ClientError::UnsupportedDownloadDirectory.into());
    }
    let client = HttpClientWrapper::new();
    let personal = PersonalClient::connect(config, &client, ListingRetry::None).await?;
    let (entry, size) = dirs::resolve_entry(&personal, &remote).await?;
    if !entry.folder {
        return Ok(download_personal(&personal, &remote, &entry.id, local, pb, size).await?);
    }
    if !recursive {
        return Err(ClientError::UnsupportedDownloadDirectory.into());
    }
    let entries = dirs::tree_with_listing(&personal, remote.clone(), entry.id).await?;
    let destination_root = Path::new(local);
    let mut plan = Vec::with_capacity(entries.len());
    // 写入文件前检查所有目标路径。
    for (entry, id) in entries {
        let rel = entry
            .path
            .strip_prefix(remote.trim_end_matches('/'))
            .unwrap()
            .trim_start_matches('/');
        let rel = PathBuf::from(rel);
        let dest = destination_root.join(&rel);
        safe_local(&dest)?;
        if dest.exists() {
            if dest.is_dir() != entry.folder {
                return Err(invalid("本地文件与目录冲突").into());
            }
            if !entry.folder {
                local_file_metadata(&dest)?;
            }
        }
        plan.push((entry, id, dest));
    }
    if !plan.is_empty() {
        let paths = plan.iter().map(|(entry, _, destination)| {
            (
                destination
                    .strip_prefix(destination_root)
                    .expect("planned descendant"),
                entry.folder,
            )
        });
        check_destination_collisions(destination_root, paths)?;
    }
    std::fs::create_dir_all(destination_root)?;
    for (entry, id, dest) in plan {
        safe_local(&dest)?;
        if entry.folder {
            std::fs::create_dir_all(&dest)?;
        } else {
            download_to_path(&personal, &id, &dest, None, entry.size)
                .await
                .map_err(|e| {
                    anyhow::anyhow!("下载 {} 失败；此前条目可能已完成: {}", entry.path, e)
                })?;
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

    #[derive(Default)]
    struct ProgressCounter(std::sync::atomic::AtomicU64);

    impl ProgressCounter {
        fn position(&self) -> u64 {
            self.0.load(Ordering::Relaxed)
        }
    }

    impl crate::progress::TransferProgress for ProgressCounter {
        fn reset(&self) {
            self.0.store(0, Ordering::Relaxed);
        }

        fn set_length(&self, _: u64) {}

        fn inc(&self, bytes: u64) {
            self.0.fetch_add(bytes, Ordering::Relaxed);
        }

        fn finish(&self) {}
    }

    async fn test_download_file(
        url: &str,
        path: &Path,
        pb: Option<Progress>,
        size: Option<i64>,
    ) -> Result<(), ClientError> {
        let client = HttpClientWrapper::new();
        write_download(DownloadLink::for_test(&client, url), path, pb, size).await
    }

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
        let pb = Arc::new(ProgressCounter::default());

        test_download_file(&url, &dir.path().join("a"), Some(pb.clone()), None)
            .await
            .expect("first download");
        assert_eq!(pb.position(), 10);

        test_download_file(&url, &dir.path().join("b"), Some(pb.clone()), None)
            .await
            .expect("second download");
        assert_eq!(pb.position(), 10, "第二次应从头累加，不是 20");
    }

    /// 中途断流后重试：进度先归零，最终不超过文件大小。
    #[tokio::test]
    async fn partial_body_then_retry_keeps_position_within_file_size() {
        let (url, hits) = spawn_server(true, b"0123456789");
        let dir = tempfile::tempdir().expect("tmp");
        let pb = Arc::new(ProgressCounter::default());

        let target = dir.path().join("a");
        std::fs::write(&target, b"existing").unwrap();
        let first = test_download_file(&url, &target, Some(pb.clone()), None).await;
        assert!(first.is_err(), "半包应当报错");
        assert_eq!(std::fs::read(&target).unwrap(), b"existing");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
        let after_first = pb.position();

        test_download_file(&url, &target, Some(pb.clone()), None)
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

    /// 在下载成功将进度补齐到文件大小之前检查进度。
    #[tokio::test]
    async fn failed_download_resets_stale_progress_before_completion() {
        let (url, _) = spawn_server(true, b"0123456789");
        let dir = tempfile::tempdir().expect("tmp");
        let pb = Arc::new(ProgressCounter::default());
        pb.0.store(7, Ordering::Relaxed);

        let result =
            test_download_file(&url, &dir.path().join("out"), Some(pb.clone()), None).await;
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
            let err = test_download_file(&server.url("/download"), path, None, None)
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
