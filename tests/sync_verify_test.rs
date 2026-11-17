//! sync --verify 的执行层契约：删除闸门、上传失败不动旧文件、下载复用扫描 id 不发路径解析。

use cloud139::config::Config;
use cloud139::domain::{ChangeKind, SyncAction, SyncEntryKind, SyncTarget};
use cloud139::services::sync_executor::{SyncVerifyOptions, execute_verified_sync};
use cloud139::services::sync_service::SyncDiffStats;
use httpmock::Method::{GET, POST};
use httpmock::prelude::*;
use serde_json::json;
use tempfile::tempdir;

fn config_for(server: &MockServer) -> Config {
    Config {
        account: "13800138000".to_string(),
        authorization: "test-authorization".to_string(),
        storage_type: "personal_new".to_string(),
        personal_cloud_host: Some(server.base_url()),
        ..Default::default()
    }
}

fn delete_action(path: &str, file_id: &str) -> SyncAction {
    SyncAction::Delete {
        rel_path: path.to_string(),
        target: SyncTarget::Cloud,
        target_abs: format!("/remote/{path}"),
        file_id: Some(file_id.to_string()),
        kind: SyncEntryKind::File,
    }
}

/// 上传初始化失败 -> 记录 failure，且删除闸门关闭：一个都不删。
#[tokio::test]
async fn upload_failure_blocks_delete_gate_and_keeps_old_files() {
    let server = MockServer::start();
    std::fs::write(tempdir().unwrap().path().join("unused"), b"x").unwrap();
    let tmp = tempdir().unwrap();
    let local = tmp.path().join("x.bin");
    std::fs::write(&local, b"hello").unwrap();

    let _init = server.mock(|when, then| {
        when.method(POST).path("/file/create");
        then.status(200)
            .json_body(json!({"success": false, "message": "boom"}));
    });
    let trash = server.mock(|when, then| {
        when.method(POST).path("/recyclebin/batchTrash");
        then.status(200)
            .json_body(json!({"success": true, "message": "ok"}));
    });

    let actions = vec![
        SyncAction::Upload {
            rel_path: "x.bin".to_string(),
            local_abs: local,
            remote_abs: "/remote/x.bin".to_string(),
            size: 5,
            change: ChangeKind::SizeOrTime,
            parent_file_id: Some("parent-id".to_string()),
            file_id: Some("old-id".to_string()),
        },
        delete_action("old.bin", "old-id"),
    ];

    let report = execute_verified_sync(
        &config_for(&server),
        actions,
        SyncDiffStats::default(),
        SyncVerifyOptions {
            dry_run: false,
            delete: true,
            progress: false,
            print_actions: false,
        },
    )
    .await
    .unwrap();

    assert_eq!(report.failures.len(), 1, "上传初始化失败应记一条 failure");
    assert_eq!(report.deleted, 0);
    trash.assert_calls(0);
}

/// 没有失败时删除闸门打开：按冻结集合删除。
#[tokio::test]
async fn delete_gate_executes_when_no_failure() {
    let server = MockServer::start();
    let trash = server.mock(|when, then| {
        when.method(POST)
            .path("/recyclebin/batchTrash")
            .json_body(json!({"fileIds": ["old-id"]}));
        then.status(200)
            .json_body(json!({"success": true, "message": "ok"}));
    });

    let report = execute_verified_sync(
        &config_for(&server),
        vec![delete_action("old.bin", "old-id")],
        SyncDiffStats::default(),
        SyncVerifyOptions {
            dry_run: false,
            delete: true,
            progress: false,
            print_actions: false,
        },
    )
    .await
    .unwrap();

    assert_eq!(report.deleted, 1);
    assert!(report.failures.is_empty());
    trash.assert_calls(1);
}

/// 带 file_id 的下载不再按路径解析（不应出现 /file/list）。
#[tokio::test]
async fn download_with_file_id_skips_path_resolution() {
    let server = MockServer::start();

    let blob = server.mock(|when, then| {
        when.method(GET).path("/blob");
        then.status(200).body("payload");
    });
    let _url = server.mock(|when, then| {
        when.method(POST)
            .path("/file/getDownloadUrl")
            .json_body(json!({"fileId": "fid"}));
        then.status(200).json_body(json!({
            "success": true,
            "data": {"url": server.url("/blob"), "fileName": "f.bin"}
        }));
    });
    let list = server.mock(|when, then| {
        when.method(POST).path("/file/list");
        then.status(200)
            .json_body(json!({"success": true, "data": {"items": [], "nextPageCursor": ""}}));
    });

    let tmp = tempdir().unwrap();
    let local = tmp.path().join("out.bin");
    cloud139::services::download_service::download_personal_for_sync(
        &config_for(&server),
        "/sub/f.bin",
        Some("fid"),
        Some("parent-id"),
        &local.to_string_lossy(),
        None,
    )
    .await
    .unwrap();

    assert_eq!(std::fs::read(&local).unwrap(), b"payload");
    blob.assert_calls(1);
    list.assert_calls(0);
}

// ---------- 状态化假服务端：验证安全替换的请求顺序 ----------

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

const HELLO_SHA256: &str = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";

#[derive(Default)]
struct FakeState {
    temp_name: Option<String>,
    events: Vec<String>,
}

fn header_end(data: &[u8]) -> Option<usize> {
    data.windows(4).position(|w| w == b"\r\n\r\n")
}

fn content_length(headers: &str) -> usize {
    headers
        .lines()
        .find_map(|line| {
            let (k, v) = line.split_once(':')?;
            if k.eq_ignore_ascii_case("content-length") {
                v.trim().parse().ok()
            } else {
                None
            }
        })
        .unwrap_or(0)
}

fn spawn_fake() -> (String, Arc<Mutex<FakeState>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let state = Arc::new(Mutex::new(FakeState::default()));
    let shared = state.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut data = Vec::new();
            let mut buf = [0u8; 8192];
            loop {
                let n = stream.read(&mut buf).unwrap_or(0);
                if n == 0 {
                    break;
                }
                data.extend_from_slice(&buf[..n]);
                if let Some(pos) = header_end(&data) {
                    let headers = String::from_utf8_lossy(&data[..pos]).to_string();
                    if data.len() >= pos + 4 + content_length(&headers) {
                        break;
                    }
                }
            }
            let text = String::from_utf8_lossy(&data).to_string();
            let path = text
                .lines()
                .next()
                .and_then(|line| line.split_whitespace().nth(1))
                .unwrap_or("/")
                .to_string();
            let body = header_end(data.as_slice())
                .map(|pos| text[pos + 4..].to_string())
                .unwrap_or_default();
            let value: serde_json::Value =
                serde_json::from_str(&body).unwrap_or(serde_json::Value::Null);

            let response = {
                let mut st = shared.lock().unwrap();
                match path.as_str() {
                    "/file/create" => {
                        let name = value
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        st.temp_name = Some(name.clone());
                        st.events.push(format!("create:{name}"));
                        json!({"success": true}).to_string()
                    }
                    "/file/list" => {
                        let temp = st.temp_name.clone().unwrap_or_default();
                        st.events.push("list".to_string());
                        json!({
                            "success": true,
                            "data": {
                                "items": [{
                                    "fileId": "temp-id",
                                    "name": temp,
                                    "size": 5,
                                    "type": "file",
                                    "contentHash": HELLO_SHA256,
                                    "contentHashAlgorithm": "SHA256"
                                }],
                                "nextPageCursor": ""
                            }
                        })
                        .to_string()
                    }
                    "/recyclebin/batchTrash" => {
                        let id = value
                            .get("fileIds")
                            .and_then(|v| v.get(0))
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        st.events.push(format!("trash:{id}"));
                        json!({"success": true, "message": "ok"}).to_string()
                    }
                    "/file/update" => {
                        let id = value
                            .get("fileId")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let name = value
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        st.events.push(format!("update:{id}:{name}"));
                        json!({"success": true}).to_string()
                    }
                    _ => json!({"success": true}).to_string(),
                }
            };
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json;charset=UTF-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response.len(),
                response
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        }
    });
    (format!("http://{addr}"), state)
}

/// 安全替换顺序：临时名上传 -> 校验 -> 删旧 -> 改名；旧内容在删之前一直在。
#[tokio::test]
async fn safe_replace_order_uploads_temp_then_verifies_then_deletes_then_renames() {
    let (url, state) = spawn_fake();
    let tmp = tempdir().unwrap();
    let local = tmp.path().join("config");
    std::fs::write(&local, b"hello").unwrap();

    let config = Config {
        account: "13800138000".to_string(),
        authorization: "test-authorization".to_string(),
        storage_type: "personal_new".to_string(),
        personal_cloud_host: Some(url),
        ..Default::default()
    };

    let report = execute_verified_sync(
        &config,
        vec![SyncAction::Upload {
            rel_path: "config".to_string(),
            local_abs: local,
            remote_abs: "/remote/config".to_string(),
            size: 5,
            change: ChangeKind::SizeOrTime,
            parent_file_id: Some("parent-id".to_string()),
            file_id: Some("old-id".to_string()),
        }],
        SyncDiffStats::default(),
        SyncVerifyOptions {
            dry_run: false,
            delete: false,
            progress: false,
            print_actions: false,
        },
    )
    .await
    .unwrap();

    assert!(
        report.failures.is_empty(),
        "替换应成功: {:?}",
        report.failures
    );
    assert_eq!(report.uploaded, 1);
    let st = state.lock().unwrap();
    let temp = st.temp_name.clone().expect("记录到临时名");
    assert!(temp.starts_with("config.cloud139-tmp-"), "{temp}");
    assert_eq!(
        st.events,
        vec![
            format!("create:{temp}"),
            "list".to_string(),
            "trash:old-id".to_string(),
            "update:temp-id:config".to_string(),
        ],
        "顺序必须是：传临时名 -> 校验 -> 删旧 -> 改名"
    );
}

/// 新上传也会当场复核：create -> list，一致才算成功（失败不拖到下一轮）。
#[tokio::test]
async fn new_upload_is_verified_after_upload() {
    let (url, state) = spawn_fake();
    let tmp = tempdir().unwrap();
    let local = tmp.path().join("x.bin");
    std::fs::write(&local, b"hello").unwrap();

    let config = Config {
        account: "13800138000".to_string(),
        authorization: "test-authorization".to_string(),
        storage_type: "personal_new".to_string(),
        personal_cloud_host: Some(url),
        ..Default::default()
    };

    let report = execute_verified_sync(
        &config,
        vec![SyncAction::Upload {
            rel_path: "x.bin".to_string(),
            local_abs: local,
            remote_abs: "/remote/x.bin".to_string(),
            size: 5,
            change: ChangeKind::New,
            parent_file_id: Some("parent-id".to_string()),
            file_id: None,
        }],
        SyncDiffStats::default(),
        SyncVerifyOptions {
            dry_run: false,
            delete: false,
            progress: false,
            print_actions: false,
        },
    )
    .await
    .unwrap();

    assert!(
        report.failures.is_empty(),
        "新上传复核应通过: {:?}",
        report.failures
    );
    assert_eq!(report.uploaded, 1);
    let st = state.lock().unwrap();
    assert_eq!(
        st.events,
        vec!["create:x.bin".to_string(), "list".to_string()],
        "新上传后必须立刻复核（列父目录）一次"
    );
}

/// 新上传后复核大小不一致 -> 记 failure，不计成功。
#[tokio::test]
async fn new_upload_size_mismatch_is_failure() {
    let (url, state) = spawn_fake();
    let tmp = tempdir().unwrap();
    let local = tmp.path().join("x.bin");
    std::fs::write(&local, b"hello!").unwrap();

    let config = Config {
        account: "13800138000".to_string(),
        authorization: "test-authorization".to_string(),
        storage_type: "personal_new".to_string(),
        personal_cloud_host: Some(url),
        ..Default::default()
    };

    let report = execute_verified_sync(
        &config,
        vec![SyncAction::Upload {
            rel_path: "x.bin".to_string(),
            local_abs: local,
            remote_abs: "/remote/x.bin".to_string(),
            size: 6,
            change: ChangeKind::New,
            parent_file_id: Some("parent-id".to_string()),
            file_id: None,
        }],
        SyncDiffStats::default(),
        SyncVerifyOptions {
            dry_run: false,
            delete: false,
            progress: false,
            print_actions: false,
        },
    )
    .await
    .unwrap();

    assert_eq!(report.failures.len(), 1, "复核大小不一致应记 failure");
    assert_eq!(report.uploaded, 0);
    let st = state.lock().unwrap();
    assert_eq!(
        st.events,
        vec!["create:x.bin".to_string(), "list".to_string()]
    );
}

/// --json 的七字段契约（camelCase，只计文件由 stats 保证）。
#[test]
fn json_report_has_seven_fields() {
    let report = cloud139::services::sync_executor::SyncReport {
        objects: 3,
        uploaded: 1,
        skipped: 2,
        deleted: 0,
        hash_missing: 1,
        seconds: 1.5,
        failures: vec![],
    };
    let value = serde_json::to_value(&report).unwrap();
    for field in [
        "objects",
        "uploaded",
        "skipped",
        "deleted",
        "hashMissing",
        "seconds",
        "failures",
    ] {
        assert!(value.get(field).is_some(), "missing field {field}");
    }
    assert_eq!(value.as_object().unwrap().len(), 7);
}
