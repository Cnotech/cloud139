use super::*;
use cloud139::client::auth::RefreshOptions;
use httpmock::prelude::*;
use std::{fs, time::Duration};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

#[tokio::test]
async fn command_dispatch_uses_its_own_config_path() {
    use clap::Parser;
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("a.toml");
    let second = dir.path().join("b.toml");
    let mut config = expiring_config("account-a");
    config.storage_type = "group".into();
    save_config(&config, &first).unwrap();
    save_config(&expiring_config("account-b"), &second).unwrap();

    let command = || {
        crate::cli::app::Cli::parse_from(["cloud139", "sync", "cloud:/", "local", "--verify"])
            .command
    };
    let (first, second) = tokio::join!(
        crate::commands::execute(command(), Some(&first)),
        crate::commands::execute(command(), Some(&second)),
    );
    assert!(matches!(
        first.unwrap_err().downcast_ref::<ClientError>(),
        Some(ClientError::UnsupportedStorage)
    ));
    assert_eq!(
        second
            .unwrap_err()
            .downcast_ref::<crate::commands::CommandExit>()
            .unwrap()
            .code(),
        2
    );
}

async fn run_startup_refresh(
    candidates: &[PathBuf],
    url: &str,
    timeout: Duration,
) -> Result<(), ClientError> {
    let Ok((config, path)) = Config::load_from_candidates(candidates) else {
        return Ok(());
    };
    config.require_personal()?;
    if config.is_token_refresh_due() {
        refresh_saved_token(
            &path,
            RefreshOptions {
                endpoint: Some(url),
                timeout,
            },
        )
        .await?;
    }
    Ok(())
}

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
    save_config(&expiring_config("account-a"), &path).unwrap();
    let original = fs::read(&path).unwrap();
    let (url, accepted, release_tx, task) = server(response).await;
    let paths = vec![path.clone()];
    let start = std::time::Instant::now();
    let operation =
        tokio::spawn(
            async move { run_startup_refresh(&paths, &url, Duration::from_millis(250)).await },
        );
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
    save_config(&expiring_config("account-b"), &path).unwrap();
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
    save_config(&expiring_config("account-a"), &global).unwrap();
    let (url, accepted, release, task) = server(success_response()).await;
    let paths = vec![local.clone(), global.clone()];
    let operation =
        tokio::spawn(
            async move { run_startup_refresh(&paths, &url, Duration::from_secs(2)).await },
        );
    tokio::time::timeout(Duration::from_secs(2), accepted)
        .await
        .unwrap()
        .unwrap();
    save_config(&expiring_config("account-b"), &local).unwrap();
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
    save_config(&expiring_config("account-a"), &path).unwrap();
    let (url, accepted, release, task) = server(success_response()).await;
    let paths = vec![path.clone()];
    let operation =
        tokio::spawn(
            async move { run_startup_refresh(&paths, &url, Duration::from_secs(2)).await },
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
    save_config(&expiring_config("account-a"), &path).unwrap();
    let (url, accepted, release, task) = server(success_response()).await;
    let paths = vec![path.clone()];
    let first_paths = paths.clone();
    let first_url = url.clone();
    let first = tokio::spawn(async move {
        run_startup_refresh(&first_paths, &first_url, Duration::from_secs(2)).await
    });
    tokio::time::timeout(Duration::from_secs(2), accepted)
        .await
        .unwrap()
        .unwrap();
    tokio::time::timeout(
        Duration::from_millis(250),
        run_startup_refresh(&paths, &url, Duration::from_secs(2)),
    )
    .await
    .unwrap()
    .unwrap();
    // 其他正常写入路径也遵守同一把锁。
    assert!(matches!(
        save_config(&expiring_config("account-b"), &path),
        Err(ConfigWriteError::Busy)
    ));
    release.send(()).unwrap();
    first.await.unwrap().unwrap();
    // 完成后重新读取新到期时间，不会用旧快照再次续期。
    tokio::time::timeout(
        Duration::from_millis(250),
        run_startup_refresh(&paths, &url, Duration::from_secs(2)),
    )
    .await
    .unwrap()
    .unwrap();
    task.abort();
}

#[tokio::test]
async fn startup_skips_missing_or_invalid_config_without_request() {
    for content in [None, Some("invalid TOML ["), Some("account = 'incomplete'")] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        if let Some(content) = content {
            fs::write(&path, content).unwrap();
        }
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST).path("/refresh");
            then.status(500);
        });
        run_startup_refresh(
            std::slice::from_ref(&path),
            &server.url("/refresh"),
            Duration::from_secs(1),
        )
        .await
        .unwrap();
        mock.assert_calls(0);
        assert_eq!(fs::read_to_string(&path).ok().as_deref(), content);
    }
}

#[tokio::test]
async fn startup_skips_fresh_config_without_rewriting_or_request() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let mut config = expiring_config("account-a");
    config.token_expire_time = Some(chrono::Utc::now().timestamp_millis() + 30 * 86_400_000);
    save_config(&config, &path).unwrap();
    let original = fs::read(&path).unwrap();
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST).path("/refresh");
        then.status(500);
    });
    run_startup_refresh(
        std::slice::from_ref(&path),
        &server.url("/refresh"),
        Duration::from_secs(1),
    )
    .await
    .unwrap();
    mock.assert_calls(0);
    assert_eq!(fs::read(&path).unwrap(), original);
}

#[tokio::test]
async fn startup_persists_refreshed_credentials_and_skips_next_refresh() {
    for expiry in [Some(chrono::Utc::now().timestamp_millis() - 1), None] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut config = expiring_config("account-a");
        config.token_expire_time = expiry;
        config.storage_type = "personal_new".into();
        config.use_large_thumbnail = true;
        save_config(&config, &path).unwrap();
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST).path("/refresh").body(
                "<root><token>old-refresh-token</token><account>account-a</account><clienttype>656</clienttype></root>",
            );
            then.status(200).body(
                "<root><return>0</return><token>new-token</token><expiretime>2592000</expiretime></root>",
            );
        });
        let before = chrono::Utc::now().timestamp_millis();
        run_startup_refresh(
            std::slice::from_ref(&path),
            &server.url("/refresh"),
            Duration::from_secs(1),
        )
        .await
        .unwrap();
        let after = chrono::Utc::now().timestamp_millis();
        let saved_content = fs::read_to_string(&path).unwrap();
        let saved: Config = toml::from_str(&saved_content).unwrap();
        config.authorization = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            "pc:account-a:new-token",
        );
        config.refresh_token = Some("new-token".into());
        let expiry = saved.token_expire_time.unwrap();
        assert!((before + 30 * 86_400_000..=after + 30 * 86_400_000).contains(&expiry));
        config.token_expire_time = Some(expiry);
        assert_eq!(
            toml::to_string(&saved).unwrap(),
            toml::to_string(&config).unwrap()
        );

        run_startup_refresh(
            std::slice::from_ref(&path),
            &server.url("/refresh"),
            Duration::from_secs(1),
        )
        .await
        .unwrap();
        mock.assert_calls(1);
        assert_eq!(fs::read_to_string(&path).unwrap(), saved_content);
    }
}

#[tokio::test]
async fn startup_refresh_failures_keep_config_and_release_lock() {
    for (body, missing_refresh_token) in [
        (
            "<root><return>4006</return><desc>expired</desc></root>",
            false,
        ),
        ("not XML", false),
        ("<root><return>0</return></root>", false),
        ("<root><return>0</return><token>unused</token></root>", true),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut config = expiring_config("account-a");
        if missing_refresh_token {
            config.refresh_token = None;
        }
        save_config(&config, &path).unwrap();
        let original = fs::read(&path).unwrap();
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST).path("/refresh");
            then.status(200).body(body);
        });
        run_startup_refresh(
            std::slice::from_ref(&path),
            &server.url("/refresh"),
            Duration::from_secs(1),
        )
        .await
        .unwrap();
        mock.assert_calls(if missing_refresh_token { 0 } else { 1 });
        assert_eq!(fs::read(&path).unwrap(), original);
        save_config(&config, &path).unwrap();
    }
}

#[tokio::test]
async fn startup_connection_failure_keeps_config_and_continues() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let config = expiring_config("account-a");
    save_config(&config, &path).unwrap();
    let original = fs::read(&path).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/refresh", listener.local_addr().unwrap());
    drop(listener);
    tokio::time::timeout(
        Duration::from_secs(2),
        run_startup_refresh(
            std::slice::from_ref(&path),
            &url,
            Duration::from_millis(250),
        ),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(fs::read(&path).unwrap(), original);
    save_config(&config, &path).unwrap();
}

#[tokio::test]
async fn startup_refresh_does_not_depend_on_legacy_sidecar_path() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let config = expiring_config("account-a");
    save_config(&config, &path).unwrap();
    let lock_path = dir.path().join("config.toml.lock");
    fs::create_dir(&lock_path).unwrap();
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST).path("/refresh");
        then.status(200).body(
            "<root><return>0</return><token>new-token</token><expiretime>2592000</expiretime></root>",
        );
    });
    run_startup_refresh(
        std::slice::from_ref(&path),
        &server.url("/refresh"),
        Duration::from_secs(1),
    )
    .await
    .unwrap();
    mock.assert_calls(1);
    let saved: Config = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(saved.refresh_token.as_deref(), Some("new-token"));
    assert!(lock_path.is_dir());
}

#[tokio::test]
async fn startup_reports_save_error_if_config_is_removed_during_refresh() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    save_config(&expiring_config("account-a"), &path).unwrap();
    let (url, accepted, release, task) = server(success_response()).await;
    let paths = vec![path.clone()];
    let operation =
        tokio::spawn(
            async move { run_startup_refresh(&paths, &url, Duration::from_secs(2)).await },
        );
    tokio::time::timeout(Duration::from_secs(2), accepted)
        .await
        .unwrap()
        .unwrap();
    fs::remove_file(&path).unwrap();
    release.send(()).unwrap();
    let error = operation.await.unwrap().unwrap_err();
    assert!(error.to_string().contains("保存配置失败"));
    assert!(!path.exists());
    save_config(&expiring_config("account-b"), &path).unwrap();
    task.abort();
}
