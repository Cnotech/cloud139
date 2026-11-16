//! mirror 的普通文件规则（不假定仓的类型）：
//! - 文件名不是 64 位十六进制 → 普通文件；远端同名但内容不同 → 失败，绝不覆盖远端。
//! - 普通文件拿不到远端 contentHash → 失败（不能当「相同」放过）。
//! - 内容寻址对象（64 位十六进制文件名）缺哈希可以接受，只计入 hashMissing。
//!
//! 用 httpmock 把列表接口的 contentHash 去掉／改掉，验证判据与「不发写请求」。

use cloud139::config::Config;
use cloud139::services::mirror_service::{self, MirrorOptions};
use httpmock::Method::POST;
use httpmock::Mock;
use httpmock::prelude::*;
use serde_json::json;
use std::fs;
use std::path::Path;

fn config_for(server: &MockServer) -> Config {
    Config {
        account: "13800138000".to_string(),
        authorization: "test-authorization".to_string(),
        storage_type: "personal_new".to_string(),
        personal_cloud_host: Some(server.base_url()),
        ..Default::default()
    }
}

fn page(items: Vec<serde_json::Value>) -> String {
    json!({
        "success": true,
        "code": "0000",
        "data": { "items": items, "nextPageCursor": "" }
    })
    .to_string()
}

fn folder(id: &str, name: &str) -> serde_json::Value {
    json!({ "fileId": id, "name": name, "type": "folder" })
}

/// 远端文件；hash=None 表示接口没返回 contentHash。
fn file(id: &str, name: &str, size: i64, hash: Option<&str>) -> serde_json::Value {
    match hash {
        Some(hash) => json!({
            "fileId": id, "name": name, "type": "file", "size": size,
            "contentHash": hash, "contentHashAlgorithm": "SHA256"
        }),
        None => json!({ "fileId": id, "name": name, "type": "file", "size": size }),
    }
}

/// 按 parentFileId 指定这一层的子项。
fn mock_list<'a>(
    server: &'a MockServer,
    parent_file_id: &str,
    items: Vec<serde_json::Value>,
) -> Mock<'a> {
    let body = page(items);
    let parent = format!("\"parentFileId\":\"{parent_file_id}\"");
    server.mock(move |when, then| {
        when.method(POST)
            .path("/file/list")
            .body_includes(parent.clone());
        then.status(200).body(body.clone());
    })
}

fn options_dry_run() -> MirrorOptions {
    MirrorOptions {
        verify: true,
        heal: 3,
        dry_run: true,
        page_size: 100,
        retries: 0,
        retry_backoff_secs: 0,
    }
}

fn write(dir: &Path, rel: &str, bytes: &[u8]) {
    let path = dir.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, bytes).unwrap();
}

/// 普通文件（config）远端没给 contentHash → 记失败、不发写请求；
/// 内容寻址对象缺哈希只计入 hashMissing。
#[tokio::test]
async fn regular_file_without_remote_hash_is_failure() {
    let server = MockServer::start();
    let addressed = "a".repeat(64);
    let root = mock_list(&server, "/", vec![folder("id-repo", "repo")]);
    let repo = mock_list(
        &server,
        "id-repo",
        vec![folder("id-data", "data"), file("id-cfg", "config", 3, None)],
    );
    let data = mock_list(&server, "id-data", vec![folder("id-aa", "aa")]);
    let aa = mock_list(&server, "id-aa", vec![file("id-obj", &addressed, 10, None)]);

    let tmp = tempfile::tempdir().unwrap();
    write(tmp.path(), "config", b"abc");
    write(tmp.path(), &format!("data/aa/{addressed}"), &[7u8; 10]);

    let report = mirror_service::run(
        &config_for(&server),
        tmp.path().to_str().unwrap(),
        "cloud:/repo",
        &options_dry_run(),
    )
    .await
    .expect("dry-run 不该变成致命错误");

    assert_eq!(report.objects, 2);
    assert_eq!(report.failures.len(), 1, "只有普通文件该失败");
    assert_eq!(report.failures[0].path, "config");
    assert!(
        report.failures[0].error.contains("远端未返回 contentHash"),
        "错误信息要说明原因，实际: {}",
        report.failures[0].error
    );
    // 内容寻址对象缺哈希不判失败，只计入 hashMissing
    assert_eq!(report.hash_missing, 1);
    assert_eq!(report.skipped, 1);
    // 没有任何修复：healed 必须为 0（旧口径会把它算成「远端已有仓 → 自愈」）
    assert_eq!(report.healed, 0);

    // 只有目录列举，没有任何上传/删除请求
    root.assert_calls(1);
    repo.assert_calls(1);
    data.assert_calls(1);
    aa.assert_calls(1);
}

/// 普通文件同名不同内容 → 失败；内容寻址对象同名不同内容 → 重传（自愈）。
#[tokio::test]
async fn conflict_handling_depends_on_content_addressing() {
    let server = MockServer::start();
    let addressed = "b".repeat(64);
    let root = mock_list(&server, "/", vec![folder("id-repo", "repo")]);
    let repo = mock_list(
        &server,
        "id-repo",
        vec![
            folder("id-data", "data"),
            file("id-cfg", "config", 3, Some(&"f".repeat(64))),
        ],
    );
    let data = mock_list(&server, "id-data", vec![folder("id-aa", "aa")]);
    let aa = mock_list(
        &server,
        "id-aa",
        vec![file("id-obj", &addressed, 10, Some(&"c".repeat(64)))],
    );

    let tmp = tempfile::tempdir().unwrap();
    write(tmp.path(), "config", b"abc");
    write(tmp.path(), &format!("data/aa/{addressed}"), &[7u8; 10]);

    let report = mirror_service::run(
        &config_for(&server),
        tmp.path().to_str().unwrap(),
        "cloud:/repo",
        &options_dry_run(),
    )
    .await
    .expect("dry-run 不该变成致命错误");

    assert_eq!(report.failures.len(), 1, "只有普通文件该失败");
    assert_eq!(report.failures[0].path, "config");
    assert!(
        report.failures[0].error.contains("远端同名但内容不同"),
        "实际: {}",
        report.failures[0].error
    );
    // 内容寻址那份走「重传」，不是失败；这是真正的修复，计入 healed
    assert_eq!(report.uploaded, 1);
    assert_eq!(report.healed, 1, "远端坏对象被重传 = 修复");

    root.assert_calls(1);
    repo.assert_calls(1);
    data.assert_calls(1);
    aa.assert_calls(1);
}

/// 正常的一夜：远端没有这个新对象 → 只算 uploaded，`healed` 必须是 0。
/// （旧口径「远端已有仓时任何上传都算 self-heal」会让正常夜也报 `healed=N`，是假警报源。）
#[tokio::test]
async fn new_object_is_uploaded_but_not_healed() {
    let server = MockServer::start();
    let addressed = "d".repeat(64);
    let root = mock_list(&server, "/", vec![folder("id-repo", "repo")]);
    let repo = mock_list(&server, "id-repo", vec![folder("id-data", "data")]);
    let data = mock_list(&server, "id-data", vec![folder("id-aa", "aa")]);
    // 远端 data/aa 是空的：本地的对象是「新增」
    let aa = mock_list(&server, "id-aa", vec![]);

    let tmp = tempfile::tempdir().unwrap();
    write(tmp.path(), &format!("data/aa/{addressed}"), &[9u8; 12]);

    let report = mirror_service::run(
        &config_for(&server),
        tmp.path().to_str().unwrap(),
        "cloud:/repo",
        &options_dry_run(),
    )
    .await
    .expect("dry-run 不该变成致命错误");

    assert_eq!(report.objects, 1);
    assert_eq!(report.uploaded, 1, "新对象要上传");
    assert_eq!(report.healed, 0, "新对象不是修复，不能计入 healed");
    assert!(report.failures.is_empty());

    root.assert_calls(1);
    repo.assert_calls(1);
    data.assert_calls(1);
    aa.assert_calls(1);
}
