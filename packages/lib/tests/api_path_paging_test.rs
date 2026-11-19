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

#[tokio::test]
async fn explicit_config_paths_keep_parallel_accounts_separate() {
    let root = tempfile::tempdir().unwrap();
    let left = MockServer::start();
    let right = MockServer::start();
    let mut configs = Vec::new();
    let mut requests = Vec::new();
    for (server, account) in [(&left, "account-a"), (&right, "account-b")] {
        let mut config = config_for(server);
        config.account = account.into();
        config.authorization = account.into();
        let path = root.path().join(format!("{account}.toml"));
        config.save_to(&path).unwrap();
        configs.push(Config::load_from_path(&path).unwrap());
        requests.push(server.mock(|when, then| {
            when.method(POST)
                .path("/file/list")
                .header("Authorization", format!("Basic {account}"));
            then.status(200)
                .body(page(vec![folder(account, "item")], ""));
        }));
    }

    let (first, second) = tokio::join!(
        get_file_id_by_path(&configs[0], "/item"),
        get_file_id_by_path(&configs[1], "/item")
    );
    assert_eq!(first.unwrap(), "account-a");
    assert_eq!(second.unwrap(), "account-b");
    for request in requests {
        request.assert_calls(1);
    }
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

#[tokio::test]
async fn all_path_lookup_entries_reject_the_root_id_as_a_child() {
    let server = MockServer::start();
    let config = config_for(&server);
    let root = server.mock(|when, then| {
        when.method(POST).path("/file/list");
        then.body(page(vec![folder("/", "alias")], ""));
    });
    let client_error = get_file_id_by_path(&config, "/alias").await.unwrap_err();
    let service_error = cloud139::services::directory_service::resolve(&config, "/alias")
        .await
        .unwrap_err();
    assert!(client_error.to_string().contains("根目录别名"));
    assert_eq!(client_error.to_string(), service_error.to_string());
    root.assert_calls(2);
}

#[tokio::test]
async fn listing_preserves_page_size_and_thumbnails_across_partial_wire_pages() {
    use cloud139::services::list_service::ListOptions;
    let server = MockServer::start();
    let mut config = config_for(&server);
    config.use_large_thumbnail = true;
    let mock_page = |cursor: &str, names: &[&str], next: &str| {
        server.mock(|when, then| {
            when.method(POST).path("/file/list").json_body_includes(
                json!({
                    "imageThumbnailStyleList": ["Small", "Large", "Original"],
                    "pageInfo": {"pageCursor": cursor, "pageSize": 2},
                })
                .to_string(),
            );
            let items = names.iter().map(|name| folder(name, name)).collect();
            then.body(page(items, next));
        })
    };
    let first = mock_page("", &["a"], "next");
    let second = mock_page("next", &["b", "c"], "last");
    let last = mock_page("last", &["d", "e"], "");
    let result = cloud139::services::list(
        &config,
        ListOptions {
            path: "/",
            page: 2,
            page_size: 2,
        },
    )
    .await
    .unwrap();
    assert_eq!(result.total, 5);
    assert_eq!(
        result
            .items
            .iter()
            .map(|item| item.name.as_str())
            .collect::<Vec<_>>(),
        ["c", "d"]
    );
    first.assert_calls(1);
    second.assert_calls(1);
    last.assert_calls(1);
}
