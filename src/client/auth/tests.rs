use super::*;
use std::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

fn expiring_config(account: &str) -> Config {
    Config {
        account: account.into(),
        authorization: "old-authorization".into(),
        refresh_token: Some("old-refresh-token".into()),
        token_expire_time: Some(chrono::Utc::now().timestamp_millis() + 60_000),
        ..Default::default()
    }
}

fn success_response() -> String {
    let body =
        "<root><return>0</return><token>new-token</token><expiretime>2592000</expiretime></root>";
    format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    )
}

// 接收完整请求后通知测试，等测试放行才回响应；不完整响应保持连接到客户端超时。
async fn server(
    response: String,
) -> (
    String,
    oneshot::Receiver<()>,
    oneshot::Sender<()>,
    tokio::task::JoinHandle<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/refresh", listener.local_addr().unwrap());
    let (accepted_tx, accepted_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    let task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        let mut buf = [0u8; 4096];
        loop {
            let n = socket.read(&mut buf).await.unwrap();
            assert!(n > 0);
            request.extend_from_slice(&buf[..n]);
            if let Some(end) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&request[..end]).to_lowercase();
                let length = headers
                    .lines()
                    .find_map(|line| {
                        line.strip_prefix("content-length:")?
                            .trim()
                            .parse::<usize>()
                            .ok()
                    })
                    .unwrap_or(0);
                if request.len() >= end + 4 + length {
                    break;
                }
            }
        }
        let _ = accepted_tx.send(());
        if release_rx.await.is_ok() {
            let _ = socket.write_all(response.as_bytes()).await;
            // keep the socket open, including for incomplete bodies
            let _ = socket.read(&mut buf).await;
        }
    });
    (url, accepted_rx, release_tx, task)
}

async fn assert_timeout_keeps_config_and_continues(response: String, release: bool) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    expiring_config("account-a").save_to(&path).unwrap();
    let original = fs::read(&path).unwrap();
    let (url, accepted, release_tx, task) = server(response).await;
    let paths = vec![path.clone()];
    let start = std::time::Instant::now();
    let operation = tokio::spawn(async move {
        maybe_refresh_config_at(&paths, &url, Duration::from_millis(250)).await
    });
    tokio::time::timeout(Duration::from_secs(2), accepted)
        .await
        .unwrap()
        .unwrap();
    if release {
        release_tx.send(()).unwrap();
    }
    // 续期失败被转换为告警，启动流程可以继续，而不是永远等待网络。
    tokio::time::timeout(Duration::from_secs(2), operation)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(start.elapsed() >= Duration::from_millis(200));
    assert_eq!(fs::read(&path).unwrap(), original);
    // 失败/超时后锁已释放，后续命令或登录可以保存配置。
    expiring_config("account-b").save_to(&path).unwrap();
    task.abort();
}

#[tokio::test]
async fn startup_continues_after_stalled_headers() {
    assert_timeout_keeps_config_and_continues(success_response(), false).await;
}

#[tokio::test]
async fn startup_continues_after_incomplete_response_body() {
    assert_timeout_keeps_config_and_continues(
        "HTTP/1.1 200 OK\r\nContent-Length: 1000\r\n\r\n<root>".into(),
        true,
    )
    .await;
}

#[tokio::test]
async fn refresh_writes_original_source_when_local_config_appears() {
    let dir = tempfile::tempdir().unwrap();
    let local = dir.path().join("local.toml");
    let global = dir.path().join("global.toml");
    expiring_config("account-a").save_to(&global).unwrap();
    let (url, accepted, release, task) = server(success_response()).await;
    let paths = vec![local.clone(), global.clone()];
    let operation =
        tokio::spawn(
            async move { maybe_refresh_config_at(&paths, &url, Duration::from_secs(2)).await },
        );
    tokio::time::timeout(Duration::from_secs(2), accepted)
        .await
        .unwrap()
        .unwrap();
    expiring_config("account-b").save_to(&local).unwrap();
    let local_content = fs::read(&local).unwrap();
    release.send(()).unwrap();
    operation.await.unwrap().unwrap();
    let refreshed: Config = toml::from_str(&fs::read_to_string(&global).unwrap()).unwrap();
    assert_eq!(refreshed.account, "account-a");
    assert_eq!(refreshed.refresh_token.as_deref(), Some("new-token"));
    assert_eq!(fs::read(&local).unwrap(), local_content);
    task.abort();
}

#[tokio::test]
async fn refresh_does_not_overwrite_external_replacement() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    expiring_config("account-a").save_to(&path).unwrap();
    let (url, accepted, release, task) = server(success_response()).await;
    let paths = vec![path.clone()];
    let operation =
        tokio::spawn(
            async move { maybe_refresh_config_at(&paths, &url, Duration::from_secs(2)).await },
        );
    tokio::time::timeout(Duration::from_secs(2), accepted)
        .await
        .unwrap()
        .unwrap();
    // 模拟不使用旁路锁的编辑器或旧版本程序。
    let replacement = toml::to_string(&expiring_config("account-b")).unwrap();
    fs::write(&path, &replacement).unwrap();
    release.send(()).unwrap();
    let error = operation.await.unwrap().unwrap_err();
    assert!(error.to_string().contains("已被其他操作修改"));
    assert_eq!(fs::read_to_string(&path).unwrap(), replacement);
    task.abort();
}

#[tokio::test]
async fn concurrent_startup_skips_busy_refresh_without_waiting() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    expiring_config("account-a").save_to(&path).unwrap();
    let (url, accepted, release, task) = server(success_response()).await;
    let paths = vec![path.clone()];
    let first_paths = paths.clone();
    let first_url = url.clone();
    let first = tokio::spawn(async move {
        maybe_refresh_config_at(&first_paths, &first_url, Duration::from_secs(2)).await
    });
    tokio::time::timeout(Duration::from_secs(2), accepted)
        .await
        .unwrap()
        .unwrap();
    tokio::time::timeout(
        Duration::from_millis(250),
        maybe_refresh_config_at(&paths, &url, Duration::from_secs(2)),
    )
    .await
    .unwrap()
    .unwrap();
    // 其他正常写入路径也遵守同一把锁。
    assert!(matches!(
        expiring_config("account-b").save_to(&path),
        Err(crate::config::ConfigError::Busy)
    ));
    release.send(()).unwrap();
    first.await.unwrap().unwrap();
    // 完成后重新读取新到期时间，不会用旧快照再次续期。
    tokio::time::timeout(
        Duration::from_millis(250),
        maybe_refresh_config_at(&paths, &url, Duration::from_secs(2)),
    )
    .await
    .unwrap()
    .unwrap();
    task.abort();
}

#[test]
fn late_cache_update_preserves_new_token_and_other_settings() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let old = expiring_config("account-a");
    old.save_to(&path).unwrap();
    let mut refreshed = old.clone();
    refreshed.authorization = "new-authorization".into();
    refreshed.refresh_token = Some("new-token".into());
    refreshed.token_expire_time = Some(chrono::Utc::now().timestamp_millis() + 30 * 86_400_000);
    refreshed.use_large_thumbnail = true;
    refreshed.root_folder_id = Some("new-root".into());
    refreshed.save_to(&path).unwrap();
    // 模拟旧命令的网络请求在自动续期完成后才返回。
    old.update_cache(&path, |current| {
        current.personal_cloud_host = Some("https://example.invalid".into());
    })
    .unwrap();
    let saved: Config = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(saved.authorization, refreshed.authorization);
    assert_eq!(saved.refresh_token, refreshed.refresh_token);
    assert_eq!(saved.token_expire_time, refreshed.token_expire_time);
    assert!(saved.use_large_thumbnail);
    assert_eq!(saved.root_folder_id, refreshed.root_folder_id);
    assert_eq!(
        saved.personal_cloud_host.as_deref(),
        Some("https://example.invalid")
    );
}

#[test]
fn late_cache_update_does_not_overwrite_another_account() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let old = expiring_config("account-a");
    expiring_config("account-b").save_to(&path).unwrap();
    let original = fs::read(&path).unwrap();
    assert!(matches!(
        old.update_cache(&path, |current| {
            current.root_folder_id = Some("wrong-account-root".into());
        }),
        Err(crate::config::ConfigError::Changed)
    ));
    assert_eq!(fs::read(&path).unwrap(), original);
}
