//! 下载前的「目标是不是目录」检查：按页找、命中即停。

use cloud139::client::ClientError;
use cloud139::config::Config;
use httpmock::Method::POST;
use httpmock::prelude::*;
use serde_json::json;

fn config_for(server: &MockServer) -> Config {
    Config {
        account: "13800138000".to_string(),
        authorization: "test-authorization".to_string(),
        storage_type: "personal_new".to_string(),
        personal_cloud_host: Some(server.base_url()),
        ..Default::default()
    }
}

fn page(items: Vec<serde_json::Value>, cursor: &str) -> String {
    json!({
        "success": true,
        "code": "0000",
        "data": { "items": items, "nextPageCursor": cursor }
    })
    .to_string()
}

fn folder(id: &str, name: &str) -> serde_json::Value {
    json!({ "fileId": id, "name": name, "type": "folder" })
}

/// 目标在第一页：解析路径时同一次列表请求就能判断类型；第二页即使返回无效 JSON 也不应被请求。
#[tokio::test]
async fn download_dir_check_stops_at_first_page() {
    let server = MockServer::start();

    let first = server.mock(|when, then| {
        when.method(POST)
            .path("/file/list")
            .body_includes("\"pageCursor\":\"\"");
        then.status(200)
            .body(page(vec![folder("id-hit", "hit")], "cursor-2"));
    });
    let second = server.mock(|when, then| {
        when.method(POST)
            .path("/file/list")
            .body_includes("cursor-2");
        then.status(200).body("not-json");
    });

    let tmp = tempfile::tempdir().expect("tmp");
    let local = tmp.path().join("out.bin");
    let result =
        cloud139::services::download(&config_for(&server), "/hit", &local.to_string_lossy(), None)
            .await;

    let err = result.expect_err("目录不应被下载");
    let client_err = err.downcast::<ClientError>().expect("client error");
    assert!(matches!(
        client_err,
        ClientError::UnsupportedDownloadDirectory
    ));
    first.assert_calls(1);
    second.assert_calls(0);
}
