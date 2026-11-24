use cloud139::client::api::files::get_personal_file_detail;
use cloud139::config::Config;
use cloud139::error::ClientError;
use cloud139::services::directory::{children, cp, directory, mv, tree};
use cloud139::services::mkdir::{MkdirMode, mkdir};
use httpmock::{Mock, prelude::*};
use serde_json::{Value, json};

fn config(server: &MockServer) -> Config {
    Config {
        personal_cloud_host: Some(server.base_url()),
        ..Config::default()
    }
}

fn list<'a>(server: &'a MockServer, parent: &str, items: Value) -> Mock<'a> {
    server.mock(|when, then| {
        when.method(POST)
            .path("/file/list")
            .json_body_includes(json!({"parentFileId": parent}).to_string());
        then.json_body(json!({"success": true, "data": {"items": items}}));
    })
}

#[tokio::test]
async fn directory_apis_resolve_and_list_the_same_tree() {
    let server = MockServer::start();
    let root = list(
        &server,
        "/",
        json!([
            {"fileId": "top", "name": "top.bin", "type": "file"},
            {"fileId": "docs", "name": "docs", "type": "folder"},
        ]),
    );
    let docs = list(
        &server,
        "docs",
        json!([{"fileId": "note", "name": "note.txt", "type": "file", "size": 4}]),
    );
    let config = config(&server);

    assert_eq!(directory(&config, "docs/").await.unwrap(), "docs");
    let error = directory(&config, "/top.bin").await.unwrap_err();
    assert!(error.to_string().contains("目标路径不是目录"));

    let items = children(&config, "/docs").await.unwrap();
    assert_eq!(items.len(), 1);
    let (name, entry, size, _) = &items[0];
    assert_eq!(name, "note.txt");
    assert_eq!(entry.id, "note");
    assert!(!entry.folder);
    assert_eq!(*size, Some(4));

    let entries = tree(&config, "/").await.unwrap();
    let entries: Vec<_> = entries
        .iter()
        .map(|entry| (entry.path.as_str(), entry.folder))
        .collect();
    assert_eq!(
        entries,
        [
            ("/docs", true),
            ("/docs/note.txt", false),
            ("/top.bin", false)
        ]
    );
    root.assert_calls(4);
    docs.assert_calls(2);
}

#[tokio::test]
async fn public_file_detail_returns_metadata_for_the_requested_id() {
    let server = MockServer::start();
    let detail = server.mock(|when, then| {
        when.method(POST)
            .path("/file/query")
            .json_body(json!({"fileId": "note"}));
        then.json_body(json!({
            "success": true,
            "data": {"fileId": "note", "name": "note.txt", "type": "file"}
        }));
    });
    let result = get_personal_file_detail(&config(&server), "note")
        .await
        .unwrap();
    assert_eq!(result.file_id, "note");
    assert_eq!(result.name, "note.txt");
    assert_eq!(result.file_type, "file");
    detail.assert_calls(1);
}

#[tokio::test]
async fn single_mkdir_rejects_existing_or_missing_parents_without_creating() {
    let server = MockServer::start();
    let root = list(
        &server,
        "/",
        json!([{"fileId": "keep", "name": "keep", "type": "folder"}]),
    );
    let create = server.mock(|when, then| {
        when.method(POST).path("/file/create");
        then.status(500);
    });
    let config = config(&server);

    let error = mkdir(&config, "/keep", MkdirMode::Single)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("目录已存在"));
    assert!(matches!(
        mkdir(&config, "/missing/child", MkdirMode::Single).await,
        Err(ClientError::FileNotFound)
    ));
    root.assert_calls(2);
    create.assert_calls(0);
}

#[tokio::test]
async fn mkdir_fails_when_created_name_cannot_be_confirmed() {
    let server = MockServer::start();
    let root = list(&server, "/", json!([]));
    let create = server.mock(|when, then| {
        when.method(POST)
            .path("/file/create")
            .json_body_includes(r#"{"parentFileId":"/","name":"new","type":"folder"}"#);
        then.json_body(json!({"success": true}));
    });

    let error = mkdir(&config(&server), "/new", MkdirMode::Single)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("创建后无法确认目录名称"));
    create.assert_calls(1);
    root.assert_calls(2);
}

#[tokio::test]
async fn transfer_rejects_duplicate_names_and_moves_into_the_source_parent() {
    let server = MockServer::start();
    let _root = list(
        &server,
        "/",
        json!([
            {"fileId": "left", "name": "left", "type": "folder"},
            {"fileId": "right", "name": "right", "type": "folder"},
            {"fileId": "dest", "name": "dest", "type": "folder"}
        ]),
    );
    let left = list(
        &server,
        "left",
        json!([{"fileId": "l", "name": "note.txt", "type": "file"}]),
    );
    let right = list(
        &server,
        "right",
        json!([{"fileId": "r", "name": "note.txt", "type": "file"}]),
    );
    let dest = list(&server, "dest", json!([]));
    let mutation = server.mock(|when, then| {
        when.method(POST).path_matches(r"^/file/batch(Copy|Move)$");
        then.status(500);
    });
    let config = config(&server);

    assert!(matches!(
        cp(&config, &[], "/dest", false).await,
        Err(ClientError::NoSourceFiles)
    ));
    let error = cp(
        &config,
        &["/left/note.txt".into(), "/right/note.txt".into()],
        "/dest",
        true,
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("多个源条目同名"), "{error}");
    let error = mv(&config, &["/left/note.txt".into()], "/left", true)
        .await
        .unwrap_err();
    assert!(
        error.to_string().contains("源目录和目标目录相同"),
        "{error}"
    );

    left.assert_calls(2);
    right.assert_calls(1);
    dest.assert_calls(1);
    mutation.assert_calls(0);
}
