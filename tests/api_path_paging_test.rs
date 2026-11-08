//! 路径解析必须翻页：父目录超过一页（>100 子项）时，目标在后续页也要能找到。

use cloud139::client::api::get_file_id_by_path;
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

fn page(items: Vec<serde_json::Value>, next_cursor: &str) -> String {
    json!({
        "success": true,
        "code": "0000",
        "data": { "items": items, "nextPageCursor": next_cursor }
    })
    .to_string()
}

fn folder(id: &str, name: &str) -> serde_json::Value {
    json!({ "fileId": id, "name": name, "type": "folder" })
}

/// 第一页 100 条不含目标，nextPageCursor 指向第二页；目标在第二页。
#[tokio::test]
async fn resolves_target_on_later_page() {
    let server = MockServer::start();

    let mut first_items = Vec::new();
    for i in 0..100 {
        first_items.push(folder(&format!("id-{i}"), &format!("d{i:03}")));
    }
    let first_body = page(first_items, "cursor-2");

    let first = server.mock(|when, then| {
        when.method(POST)
            .path("/file/list")
            .body_includes("\"pageCursor\":\"\"");
        then.status(200).body(&first_body);
    });

    let second = server.mock(|when, then| {
        when.method(POST)
            .path("/file/list")
            .body_includes("cursor-2");
        then.status(200).body(page(vec![folder("id-03", "03")], ""));
    });

    let id = get_file_id_by_path(&config_for(&server), "/03")
        .await
        .expect("target on second page should resolve");

    assert_eq!(id, "id-03");
    first.assert_calls(1);
    second.assert_calls(1);
}

/// 目标在第一页时命中即停：第二页即使返回无效 JSON 也不应被请求。
#[tokio::test]
async fn resolves_target_on_first_page_without_reading_later_pages() {
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

    let id = get_file_id_by_path(&config_for(&server), "/hit")
        .await
        .expect("first page hit should resolve without reading page 2");

    assert_eq!(id, "id-hit");
    first.assert_calls(1);
    second.assert_calls(0);
}
