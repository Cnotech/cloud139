use cloud139::config::Config;
use cloud139::models::list::PersonalFileItem;
use cloud139::models::sync_item::SyncDirection;
use cloud139::services::sync::paths::normalize_cloud_path;
use cloud139::services::sync::scan::{SyncScanOptions, personal_item_to_file_entry, scan_cloud};
use cloud139::utils::remote_path::cloud_child_path;
use httpmock::{Method::POST, MockServer};
use serde_json::json;

#[tokio::test]
async fn invalid_exclude_is_rejected_before_resolving_cloud_root() {
    let server = MockServer::start();
    let requests = server.mock(|when, then| {
        when.any_request();
        then.status(400);
    });
    let config = Config {
        personal_cloud_host: Some(server.base_url()),
        ..Config::default()
    };
    let error = scan_cloud(
        &config,
        "/folder",
        &SyncScanOptions {
            recursive: true,
            exclude: vec!["[".into()],
        },
        SyncDirection::CloudToLocal,
    )
    .await
    .err()
    .expect("无效排除规则不应开始云端扫描");
    assert!(error.to_string().contains("无效 exclude glob"));
    requests.assert_calls(0);
}

#[tokio::test]
async fn recursive_scan_rejects_missing_names_ids_and_conflicting_entries() {
    for (items, expected) in [
        (json!([{"fileId": "a", "type": "file"}]), "云端条目缺少名称"),
        (
            json!([{"name": "docs", "type": "folder"}]),
            "云端目录缺少有效 ID: docs",
        ),
        (
            json!([
                {"fileId": "a", "name": "same.txt", "type": "file"},
                {"fileId": "b", "name": "same.txt", "type": "file"},
            ]),
            "云端扫描返回了冲突的路径: same.txt",
        ),
        (
            json!([
                {"fileId": "same-id", "name": "a.txt", "type": "file"},
                {"fileId": "same-id", "name": "b.txt", "type": "file"},
            ]),
            "云端扫描返回了重复的条目 ID: b.txt",
        ),
    ] {
        let server = MockServer::start();
        let listing = server.mock(|when, then| {
            when.method(POST)
                .path("/file/list")
                .json_body_includes(r#"{"parentFileId":"/"}"#);
            then.json_body(json!({"success": true, "data": {"items": items}}));
        });
        let config = Config {
            personal_cloud_host: Some(server.base_url()),
            ..Config::default()
        };
        let error = scan_cloud(
            &config,
            "/",
            &SyncScanOptions {
                recursive: true,
                exclude: vec![],
            },
            SyncDirection::CloudToLocal,
        )
        .await
        .err()
        .expect("异常列表应使同步扫描失败");

        assert_eq!(error.to_string(), expected);
        listing.assert_calls(1);
    }
}

#[tokio::test]
async fn recursive_scan_visits_duplicate_directory_entries_once() {
    let server = MockServer::start();
    let root = server.mock(|when, then| {
        when.method(POST)
            .path("/file/list")
            .json_body_includes(r#"{"parentFileId":"/"}"#);
        let folder = json!({"fileId": "docs-id", "name": "docs", "type": "folder"});
        then.json_body(json!({"success": true, "data": {"items": [folder, folder]}}));
    });
    let nested = server.mock(|when, then| {
        when.method(POST)
            .path("/file/list")
            .json_body_includes(r#"{"parentFileId":"docs-id"}"#);
        then.json_body(json!({"success": true, "data": {"items": [
            {"fileId": "note-id", "name": "note.txt", "type": "file"}
        ]}}));
    });
    let config = Config {
        personal_cloud_host: Some(server.base_url()),
        ..Config::default()
    };
    let scan = scan_cloud(
        &config,
        "/",
        &SyncScanOptions {
            recursive: true,
            exclude: vec![],
        },
        SyncDirection::CloudToLocal,
    )
    .await
    .unwrap();

    let paths: Vec<_> = scan
        .items
        .iter()
        .map(|item| item.rel_path.as_str())
        .collect();
    assert_eq!(paths, ["docs", "docs/note.txt"]);
    root.assert_calls(1);
    nested.assert_calls(1);
}

fn item(
    name: &str,
    size: Option<i64>,
    file_type: &str,
    updated_at: Option<&str>,
) -> PersonalFileItem {
    PersonalFileItem {
        file_id: Some(format!("id-{name}")),
        name: Some(name.to_string()),
        size,
        file_type: Some(file_type.to_string()),
        created_at: None,
        updated_at: updated_at.map(str::to_string),
        create_date: None,
        update_date: None,
        last_modified: None,
        thumbnail_urls: None,
        content_hash: None,
        content_hash_algorithm: None,
    }
}

#[test]
fn test_should_treat_personal_item_as_folder_accepts_known_folder_values() {
    assert!(item("docs", None, "folder", None).is_folder());
    assert!(item("docs", None, "dir", None).is_folder());
    assert!(item("docs", None, "1", None).is_folder());
    assert!(!item("readme.md", Some(3), "file", None).is_folder());
}

#[test]
fn test_cloud_child_path_joins_root_and_nested_paths() {
    assert_eq!(cloud_child_path("/", "docs"), "/docs");
    assert_eq!(cloud_child_path("/remote", "docs"), "/remote/docs");
    assert_eq!(
        cloud_child_path("/remote/docs", "readme.md"),
        "/remote/docs/readme.md"
    );
}

#[test]
fn test_personal_item_to_file_entry_uses_relative_path_and_mtime() {
    let entry = personal_item_to_file_entry(
        "docs",
        &item("readme.md", Some(42), "file", Some("2024-01-01T00:00:03Z")),
    )
    .unwrap();

    assert_eq!(entry.rel_path, "docs/readme.md");
    assert_eq!(entry.size, 42);
    assert_eq!(entry.mtime, Some(1704067203));
}

#[test]
fn test_normalize_cloud_path_maps_root_variants() {
    assert_eq!(normalize_cloud_path(""), "/");
    assert_eq!(normalize_cloud_path("/"), "/");
    assert_eq!(normalize_cloud_path("docs"), "/docs");
    assert_eq!(normalize_cloud_path("/docs"), "/docs");
    assert_eq!(normalize_cloud_path("docs/"), "/docs");
    assert_eq!(normalize_cloud_path("/docs/"), "/docs");
}

#[test]
fn test_cloud_child_path_root_produces_valid_absolute_paths() {
    assert_eq!(cloud_child_path("/", "readme.md"), "/readme.md");
    assert_eq!(cloud_child_path("", "readme.md"), "/readme.md");
}

#[test]
fn test_personal_item_to_file_entry_top_level_has_no_prefix_dot() {
    let entry = personal_item_to_file_entry(
        "",
        &item("top.txt", Some(10), "file", Some("2024-06-01T00:00:00Z")),
    )
    .unwrap();

    assert_eq!(entry.rel_path, "top.txt");
}
