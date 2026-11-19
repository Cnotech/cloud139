//! Cursor cycles must fail explicitly, not loop or report a partial scan as complete.
use cloud139::client::{api::get_file_id_by_path, http::HttpClientWrapper};
use cloud139::services::sync_service::{SyncScanOptions, scan_cloud};
use cloud139::{config::Config, domain::sync_item::SyncDirection};
use httpmock::prelude::*;
use serde_json::json;
use std::time::Duration;

fn config_for(server: &MockServer) -> Config {
    Config {
        account: "13800138000".to_string(),
        authorization: "test-authorization".to_string(),
        storage_type: "personal_new".to_string(),
        personal_cloud_host: Some(server.base_url()),
        ..Default::default()
    }
}

async fn assert_cycle_fails(operation: &str, cycle: bool) {
    let server = MockServer::start();
    let config = config_for(&server);
    let cursors = if cycle {
        vec![("", "a"), ("a", "b"), ("b", "a")]
    } else {
        vec![("", "a"), ("a", "a")]
    };
    let mocks: Vec<_> = cursors
        .iter()
        .map(|(cursor, next)| {
            server.mock(|when, then| {
                when.method(POST)
                    .path("/file/list")
                    .body_includes(format!("\"pageCursor\":\"{cursor}\""));
                then.status(200).json_body(json!({
                    "success": true,
                    "data": {
                        "items": [{"fileId":"other-id", "name":"other", "type":"file", "size":1}],
                        "nextPageCursor": next
                    }
                }));
            })
        })
        .collect();
    let result = tokio::time::timeout(Duration::from_secs(3), async {
        match operation {
            "lookup" => get_file_id_by_path(&config, "/missing")
                .await
                .map(|_| ())
                .map_err(anyhow::Error::from),
            "list" => HttpClientWrapper::new()
                .list_personal_files(&config, "/")
                .await
                .map(|_| ())
                .map_err(anyhow::Error::from),
            "sync" => scan_cloud(
                &config,
                "/",
                &SyncScanOptions {
                    recursive: true,
                    exclude: vec![],
                },
                SyncDirection::CloudToLocal,
            )
            .await
            .map(|_| ()),
            _ => unreachable!(),
        }
    })
    .await
    .expect("cursor cycle must terminate promptly");
    let err = result.expect_err("a cyclic listing must not succeed with partial results");
    assert!(err.to_string().contains("分页游标重复"), "{err:#}");
    assert!(!cloud139::utils::retry::is_transient(&err));
    for mock in mocks {
        mock.assert_calls(1);
    }
}

#[tokio::test]
async fn lookup_rejects_repeated_cursor() {
    assert_cycle_fails("lookup", false).await;
}
#[tokio::test]
async fn lookup_rejects_cursor_cycle() {
    assert_cycle_fails("lookup", true).await;
}
#[tokio::test]
async fn list_rejects_repeated_cursor() {
    assert_cycle_fails("list", false).await;
}
#[tokio::test]
async fn list_rejects_cursor_cycle() {
    assert_cycle_fails("list", true).await;
}
#[tokio::test]
async fn sync_rejects_repeated_cursor() {
    assert_cycle_fails("sync", false).await;
}
#[tokio::test]
async fn sync_rejects_cursor_cycle() {
    assert_cycle_fails("sync", true).await;
}
