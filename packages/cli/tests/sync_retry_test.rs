//! sync 的请求级重试：列表断连一次后就地重试，整轮不中断。
//!
//! 实际使用中出现过的现象：`error sending request ... /file/list` → 整轮失败。
//! 现在同一个分页请求会自己重试，日志里看到「操作失败，N 秒后重试」后继续往下走。

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

/// 第一次请求直接断连（模拟真实网络里的 `error sending request`），之后返回空列表。
fn spawn_flaky_server() -> (String, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let hits = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&hits);

    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut buf = [0u8; 8192];
            let _ = stream.read(&mut buf);
            let n = counter.fetch_add(1, Ordering::SeqCst);

            // 第一次直接断开连接（模拟真实断连），不返回任何响应
            if n == 0 {
                drop(stream);
                continue;
            }

            let (status, body) = (
                "200 OK",
                r#"{"success":true,"code":"0000","data":{"items":[],"nextPageCursor":""}}"#,
            );

            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        }
    });

    (format!("http://{addr}"), hits)
}

fn write_config(dir: &std::path::Path, host: &str) -> std::path::PathBuf {
    let path = dir.join("cloud139rc.toml");
    let content = format!(
        "authorization = \"test-authorization\"\naccount = \"13800138000\"\nstorage_type = \"personal_new\"\npersonal_cloud_host = \"{host}\"\ntoken_expire_time = 9999999999999\n"
    );
    std::fs::write(&path, content).expect("write config");
    path
}

/// 列目录第一次 500、第二次 200：sync 应当就地重试并成功，退出码 0。
#[test]
fn sync_retries_list_request_and_succeeds() {
    let (host, hits) = spawn_flaky_server();
    let tmp = tempfile::tempdir().expect("tmp");
    let config = write_config(tmp.path(), &host);
    let empty_local = tmp.path().join("empty");
    std::fs::create_dir_all(&empty_local).expect("dir");

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_cloud139"))
        .arg("-c")
        .arg(&config)
        .arg("sync")
        .arg(&empty_local)
        .arg("cloud:/")
        .output()
        .expect("run cloud139");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");
    assert_eq!(
        output.status.code(),
        Some(0),
        "断连一次后应重试成功，输出:\n{combined}"
    );
    // 日志里要能看到的那一行（这版 CLI 的日志走 stdout）
    assert!(
        combined.contains("操作失败") && combined.contains("重试"),
        "应当出现请求级重试日志，实际:\n{combined}"
    );
    assert_eq!(
        hits.load(Ordering::SeqCst),
        2,
        "应当正好重试一次：首次 500 + 重试 200"
    );
}

/// 所有请求都返回同一段 JSON（验证确定性错误不重试）。
fn spawn_static_server(body: &'static str) -> (String, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let hits = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&hits);
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut buf = [0u8; 8192];
            let _ = stream.read(&mut buf);
            counter.fetch_add(1, Ordering::SeqCst);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        }
    });
    (format!("http://{addr}"), hits)
}

/// 完整但与模型不兼容的 JSON 是确定性错误：只请求一次，不重试。
#[test]
fn sync_does_not_retry_incompatible_json() {
    let (host, hits) = spawn_static_server(
        r#"{"success":true,"code":"0000","data":{"items":"not-a-list","nextPageCursor":""}}"#,
    );
    let tmp = tempfile::tempdir().expect("tmp");
    let config = write_config(tmp.path(), &host);
    let empty_local = tmp.path().join("empty");
    std::fs::create_dir_all(&empty_local).expect("dir");

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_cloud139"))
        .arg("-c")
        .arg(&config)
        .arg("sync")
        .arg(&empty_local)
        .arg("cloud:/")
        .output()
        .expect("run cloud139");

    assert_eq!(
        output.status.code(),
        Some(2),
        "确定性解码错误应当直接失败（退出码 2）"
    );
    assert_eq!(
        hits.load(Ordering::SeqCst),
        1,
        "解码错误不应重试，只应请求一次"
    );
}
