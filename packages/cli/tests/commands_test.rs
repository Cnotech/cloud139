use cloud139::config::Config;
use httpmock::{Mock, prelude::*};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn command(profile: &Path, args: &[&str]) -> Output {
    let root = profile.parent().unwrap();
    Command::new(env!("CARGO_BIN_EXE_cloud139"))
        .current_dir(root)
        .env("HOME", root)
        .env("XDG_CONFIG_HOME", root)
        .env("USERPROFILE", root)
        .arg("--config")
        .arg(profile)
        .args(args)
        .output()
        .unwrap()
}

fn write_profile(root: &Path, server: &MockServer) -> PathBuf {
    let profile = root.join("profile.toml");
    Config {
        personal_cloud_host: Some(server.base_url()),
        token_expire_time: Some(i64::MAX),
        ..Config::default()
    }
    .save_to(&profile)
    .unwrap();
    profile
}

fn mock_list<'a>(server: &'a MockServer, parent: &str, items: Value) -> Mock<'a> {
    server.mock(|when, then| {
        when.method(POST)
            .path("/file/list")
            .json_body_includes(json!({"parentFileId": parent}).to_string());
        then.json_body(json!({"success": true, "data": {"items": items}}));
    })
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn ls_renders_entries_and_writes_json_for_both_listing_modes() {
    let server = MockServer::start();
    let temp = tempfile::tempdir().unwrap();
    let profile = write_profile(temp.path(), &server);
    let root = mock_list(
        &server,
        "/",
        json!([
            {"fileId": "docs", "name": "docs", "type": "folder"},
            {"fileId": "file", "name": "note.txt", "type": "file", "size": 2048,
             "contentHash": "example-hash", "contentHashAlgorithm": "SHA256"},
        ]),
    );
    let docs = mock_list(&server, "docs", json!([]));

    let output = command(&profile, &["ls", "/", "-s", "10", "-o", "page.json"]);
    assert_success(&output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.lines().any(|line| line.starts_with("d docs")));
    assert!(
        stdout
            .lines()
            .any(|line| line.starts_with("- note.txt") && line.contains("2.00 KB"))
    );
    let page: Value =
        serde_json::from_slice(&std::fs::read(temp.path().join("page.json")).unwrap()).unwrap();
    assert_eq!(page["page"], 1);
    assert_eq!(page["page_size"], 10);
    assert_eq!(page["total"], 2);
    assert_eq!(page["items"][0]["type"], "folder");
    assert!(page["items"][0].get("content_hash").is_none());
    assert_eq!(page["items"][1]["type"], "file");
    assert_eq!(page["items"][1]["content_hash"], "example-hash");

    let output = command(&profile, &["ls", "-R", "/", "-o", "tree.json"]);
    assert_success(&output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("文件列表 (/docs)"));
    let tree: Value =
        serde_json::from_slice(&std::fs::read(temp.path().join("tree.json")).unwrap()).unwrap();
    assert_eq!(tree[0]["path"], "/");
    assert!(tree[0].get("page").is_none());
    assert_eq!(tree[1]["path"], "/docs");
    assert_eq!(tree[1]["items"], json!([]));
    root.assert_calls(2);
    docs.assert_calls(1);
}

#[test]
fn mkdir_parents_accepts_existing_directory_and_single_mode_rejects_it() {
    let server = MockServer::start();
    let temp = tempfile::tempdir().unwrap();
    let profile = write_profile(temp.path(), &server);
    let root = mock_list(
        &server,
        "/",
        json!([{"fileId":"docs","name":"docs","type":"folder"}]),
    );
    let create = server.mock(|when, then| {
        when.method(POST).path("/file/create");
        then.status(500);
    });

    assert_success(&command(&profile, &["mkdir", "-p", "/docs"]));
    let output = command(&profile, &["mkdir", "/docs"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("目录已存在"));
    root.assert_calls(2);
    create.assert_calls(0);
}

#[test]
fn rm_requires_confirmation_before_resolving_and_trashing_a_file() {
    let server = MockServer::start();
    let temp = tempfile::tempdir().unwrap();
    let profile = write_profile(temp.path(), &server);
    let root = mock_list(
        &server,
        "/",
        json!([{"fileId":"file","name":"note.txt","type":"file"}]),
    );
    let trash = server.mock(|when, then| {
        when.method(POST)
            .path("/recyclebin/batchTrash")
            .json_body(json!({"fileIds":["file"]}));
        then.json_body(json!({"success": true}));
    });

    let output = command(&profile, &["rm", "/note.txt"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stdout).contains("--yes"));
    root.assert_calls(0);
    trash.assert_calls(0);

    assert_success(&command(&profile, &["rm", "--yes", "/note.txt"]));
    root.assert_calls(1);
    trash.assert_calls(1);
}

#[test]
fn copy_move_and_rename_send_the_expected_file_id() {
    let server = MockServer::start();
    let temp = tempfile::tempdir().unwrap();
    let profile = write_profile(temp.path(), &server);
    let _root = mock_list(
        &server,
        "/",
        json!([
            {"fileId": "docs", "name": "docs", "type": "folder"},
            {"fileId": "file", "name": "note.txt", "type": "file"}
        ]),
    );
    let _docs = mock_list(&server, "docs", json!([]));
    for (args, endpoint, body) in [
        (
            ["cp", "/note.txt", "/docs"],
            "/file/batchCopy",
            json!({"fileIds":["file"],"toParentFileId":"docs"}),
        ),
        (
            ["mv", "/note.txt", "/docs"],
            "/file/batchMove",
            json!({"fileIds":["file"],"toParentFileId":"docs"}),
        ),
        (
            ["rename", "/note.txt", "new.txt"],
            "/file/update",
            json!({"fileId":"file","name":"new.txt"}),
        ),
    ] {
        let request = server.mock(|when, then| {
            when.method(POST)
                .path(endpoint)
                .json_body_includes(body.to_string());
            then.json_body(json!({"success": true}));
        });
        assert_success(&command(&profile, &args));
        request.assert_calls(1);
    }
}

#[test]
fn invalid_login_preserves_existing_profile() {
    let temp = tempfile::tempdir().unwrap();
    let profile = temp.path().join("profile.toml");
    std::fs::write(&profile, "原有配置").unwrap();
    let output = command(&profile, &["login", "-t", "!"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("Token 格式错误"));
    assert_eq!(std::fs::read_to_string(profile).unwrap(), "原有配置");
}

#[test]
fn sync_preview_displays_text_summary_in_both_modes_without_uploading() {
    let server = MockServer::start();
    let temp = tempfile::tempdir().unwrap();
    let profile = write_profile(temp.path(), &server);
    let source = temp.path().join("source");
    std::fs::create_dir(&source).unwrap();
    std::fs::write(source.join("note.txt"), "test").unwrap();
    let root = mock_list(&server, "/", json!([]));
    let create = server.mock(|when, then| {
        when.method(POST).path("/file/create");
        then.status(500);
    });

    let summary = command(&profile, &["sync", "-n", "source", "cloud:/"]);
    assert_success(&summary);
    let stdout = String::from_utf8(summary.stdout).unwrap();
    assert!(stdout.contains("(DRY RUN)"));
    assert!(stdout.contains("note.txt"));
    assert!(stdout.contains("同步计划: 1 个文件传输"));

    let verify = command(&profile, &["sync", "-n", "--verify", "source", "cloud:/"]);
    assert_success(&verify);
    let stdout = String::from_utf8(verify.stdout).unwrap();
    assert!(stdout.contains("校验计划: 1 个源文件，1 个上传"));
    root.assert_calls(2);
    create.assert_calls(0);
}
