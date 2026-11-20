//! 使用本机回环接口的有状态回归测试，仅使用模拟凭据和接口。
use cloud139::utils::remote_path;
use cloud139::{config::Config, error::ClientError, services};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

#[derive(Default)]
struct State {
    children: HashMap<String, Vec<Value>>,
    requests: Vec<(String, Value)>,
    fail_name: Option<String>,
    fail_upload: bool,
    fail_rename_to: Option<String>,
    sync_mutations: bool,
    reuse_existing_upload_id: bool,
    mutate_source_on_list: Option<(usize, std::path::PathBuf, &'static [u8])>,
    multipart_content: Option<&'static [u8]>,
    staged_id_mismatch: bool,
    omit_staged_id: bool,
    omit_upload_receipt: bool,
    replace_after_scan: Option<(&'static str, &'static str)>,
    staged_visibility_delay: usize,
    fail_batch: bool,
    fail_trash_item: bool,
    page_size: usize,
    cycle: bool,
    drop_list_cursor_once: Option<String>,
    close_delimited_payload: Option<&'static [u8]>,
    file_payloads: HashMap<String, &'static [u8]>,
    uploaded_content: Option<(&'static [u8], bool)>,
    fail_download: Option<String>,
    replace_extra_on_promotion: bool,
    replace_extra_after_trash: bool,
    move_parent_on_stage: bool,
}
struct Server {
    host: String,
    state: Arc<Mutex<State>>,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}
impl Server {
    fn new() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let host = format!("http://{}", listener.local_addr().unwrap());
        let state = Arc::new(Mutex::new(State {
            page_size: 100,
            ..Default::default()
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let (shared, stopped, base) = (state.clone(), stop.clone(), host.clone());
        let worker = thread::spawn(move || {
            while !stopped.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        stream.set_nonblocking(false).unwrap();
                        stream
                            .set_read_timeout(Some(Duration::from_secs(3)))
                            .unwrap();
                        let Some((path, body)) = read_request(&mut stream) else {
                            continue;
                        };
                        let mut state = shared.lock().unwrap();
                        state.requests.push((path.clone(), body.clone()));
                        let response = match path.as_str() {
                            "/file/list" => {
                                if let Some((request, _, _)) = &state.mutate_source_on_list {
                                    let count = state
                                        .requests
                                        .iter()
                                        .filter(|(path, _)| path == "/file/list")
                                        .count();
                                    if count == *request {
                                        let (_, path, contents) =
                                            state.mutate_source_on_list.take().unwrap();
                                        std::fs::write(path, contents).unwrap();
                                    }
                                }
                                let parent = body["parentFileId"].as_str().unwrap();
                                let cursor = body["pageInfo"]["pageCursor"].as_str().unwrap_or("");
                                if state.drop_list_cursor_once.as_deref() == Some(cursor) {
                                    state.drop_list_cursor_once = None;
                                    continue;
                                }
                                let offset = body["pageInfo"]["pageCursor"]
                                    .as_str()
                                    .unwrap_or("")
                                    .parse::<usize>()
                                    .unwrap_or(0);
                                let mut items =
                                    state.children.get(parent).cloned().unwrap_or_default();
                                if state.staged_visibility_delay > 0
                                    && items.iter().any(|item| {
                                        item["name"].as_str().is_some_and(|name| {
                                            name.starts_with(".cloud139-upload-")
                                        })
                                    })
                                {
                                    state.staged_visibility_delay -= 1;
                                    items.retain(|item| {
                                        !item["name"].as_str().is_some_and(|name| {
                                            name.starts_with(".cloud139-upload-")
                                        })
                                    });
                                }
                                if let Some((previous, replacement)) =
                                    state.replace_after_scan.take()
                                    && let Some(children) = state.children.get_mut(parent)
                                    && let Some(item) =
                                        children.iter_mut().find(|item| item["fileId"] == previous)
                                {
                                    item["fileId"] = json!(replacement);
                                }
                                let end = (offset + state.page_size).min(items.len());
                                let next = if state.cycle {
                                    "1".into()
                                } else if end < items.len() {
                                    end.to_string()
                                } else {
                                    String::new()
                                };
                                json!({"success":true,"data":{"items":items.get(offset..end).unwrap_or_default(),"nextPageCursor":next}})
                            }
                            "/file/create" => {
                                if (state.fail_upload && body["type"] == "file")
                                    || state
                                        .fail_name
                                        .as_deref()
                                        .is_some_and(|name| body["name"] == name)
                                {
                                    json!({"success":false,"code":"04010319","message":"injected failure"})
                                } else if state.reuse_existing_upload_id && body["type"] == "file" {
                                    // 成功回执误用已有文件的 ID 时，必须保护该文件。
                                    json!({"success":true,"data":{"fileId":"good-id","fileName":body["name"],"exist":true,"partInfos":[]}})
                                } else {
                                    let id = format!("id-{}", state.requests.len());
                                    let staged_id = if state.staged_id_mismatch {
                                        "different-id"
                                    } else {
                                        &id
                                    };
                                    let parent = body["parentFileId"].as_str().unwrap().to_owned();
                                    let mut item = json!({"fileId":staged_id,"name":body["name"],"type":body["type"],"size":body["size"]});
                                    if body["type"] == "file"
                                        && let Some((bytes, publish_hash)) = state.uploaded_content
                                    {
                                        state.file_payloads.insert(staged_id.to_owned(), bytes);
                                        if publish_hash {
                                            let hash = hash_bytes(bytes);
                                            item["contentHash"] = json!(hash);
                                            item["contentHashAlgorithm"] = json!("SHA256");
                                        }
                                        item["size"] = json!(bytes.len());
                                    }
                                    state.children.entry(parent.clone()).or_default().push(item);
                                    if state.move_parent_on_stage
                                        && body["type"] == "file"
                                        && let Some(root) = state.children.get_mut("/")
                                        && let Some(directory) =
                                            root.iter_mut().find(|item| item["fileId"] == parent)
                                    {
                                        directory["name"] = json!("moved");
                                    }
                                    if state.omit_upload_receipt {
                                        json!({"success": true})
                                    } else if body["type"] == "file"
                                        && state.multipart_content.is_some()
                                    {
                                        json!({"success":true,"data":{"fileId":id,"fileName":body["name"],"exist":false,"uploadId":"test-upload","partInfos":[{"partNumber":1,"uploadUrl":format!("{base}/part/{id}")}]}})
                                    } else {
                                        // 秒传仅凭哈希确认文件并返回上传回执。
                                        let file_id = (!state.omit_staged_id).then_some(&id);
                                        json!({"success":true,"data":{"fileId":file_id,"fileName":body["name"],"exist":true,"partInfos":[]}})
                                    }
                                }
                            }
                            path if path.starts_with("/part/") => {
                                let id = path.trim_start_matches("/part/");
                                let content = state.multipart_content.unwrap();
                                assert_eq!(body.as_str().unwrap().as_bytes(), content);
                                for items in state.children.values_mut() {
                                    for item in items.iter_mut().filter(|item| item["fileId"] == id)
                                    {
                                        item["size"] = json!(content.len());
                                        item["contentHash"] = json!(hash_bytes(content));
                                        item["contentHashAlgorithm"] = json!("SHA256");
                                    }
                                }
                                state.file_payloads.insert(id.to_owned(), content);
                                json!({"success":true})
                            }
                            "/file/complete" => json!({"success":true}),
                            "/file/getDownloadUrl" => {
                                let id = body["fileId"].as_str().unwrap();
                                if state.fail_download.as_deref() == Some(id) {
                                    json!({"success":false,"message":"injected download failure","data":{}})
                                } else if state.file_payloads.contains_key(id) {
                                    json!({"success":true,"data":{"url":format!("{base}/payload/{id}"),"fileName":"safe.bin"}})
                                } else {
                                    json!({"success":true,"data":{"url":format!("{base}/payload"),"fileName":"safe.bin"}})
                                }
                            }
                            path if path.starts_with("/payload/") => {
                                let id = path.trim_start_matches("/payload/");
                                let payload = state.file_payloads[id];
                                drop(state);
                                let _ = write!(
                                    stream,
                                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                    payload.len()
                                );
                                let _ = stream.write_all(payload);
                                continue;
                            }
                            "/payload" => {
                                let payload = state.close_delimited_payload;
                                drop(state);
                                if let Some(payload) = payload {
                                    let _ = stream
                                        .write_all(b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n");
                                    let _ = stream.write_all(payload);
                                } else {
                                    let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\ndata");
                                }
                                continue;
                            }
                            "/file/batchCopy" | "/file/batchMove" => {
                                json!({"success":!state.fail_batch,"message":"injected batch failure"})
                            }
                            "/file/update" => {
                                if state.fail_rename_to.as_deref() == body["name"].as_str() {
                                    json!({"success":false,"message":"injected rename failure"})
                                } else {
                                    if state.sync_mutations {
                                        for items in state.children.values_mut() {
                                            for item in items.iter_mut() {
                                                if item["fileId"] == body["fileId"] {
                                                    item["name"] = body["name"].clone();
                                                }
                                            }
                                        }
                                    }
                                    if state.replace_extra_on_promotion
                                        && body["name"] == "movie.bin"
                                    {
                                        for item in state.children.get_mut("/").unwrap() {
                                            if item["fileId"] == "extra" {
                                                item["fileId"] = json!("different-extra");
                                            }
                                        }
                                    }
                                    json!({"success":true})
                                }
                            }
                            "/recyclebin/batchTrash" => {
                                if state.sync_mutations && !state.fail_trash_item {
                                    let ids = body["fileIds"].as_array().unwrap();
                                    for items in state.children.values_mut() {
                                        items.retain(|item| !ids.contains(&item["fileId"]));
                                    }
                                    if state.replace_extra_after_trash
                                        && let Some(item) =
                                            state.children.get_mut("/").and_then(|items| {
                                                items
                                                    .iter_mut()
                                                    .find(|item| item["fileId"] == "extra")
                                            })
                                    {
                                        item["fileId"] = json!("different-extra");
                                    }
                                }
                                json!({"success":true,"data":{"results":[{"success":!state.fail_trash_item}]}})
                            }
                            _ => json!({"success":false,"message":"unknown mock endpoint"}),
                        };
                        let body = response.to_string();
                        let header = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        );
                        let _ = stream.write_all(header.as_bytes());
                        let _ = stream.write_all(body.as_bytes());
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2))
                    }
                    Err(_) => break,
                }
            }
        });
        Self {
            host,
            state,
            stop,
            worker: Some(worker),
        }
    }
    fn config(&self) -> Config {
        Config {
            account: "13800138000".into(),
            authorization: "test-authorization".into(),
            storage_type: "personal_new".into(),
            personal_cloud_host: Some(self.host.clone()),
            ..Default::default()
        }
    }
    fn set(&self, parent: &str, items: Vec<Value>) {
        self.state
            .lock()
            .unwrap()
            .children
            .insert(parent.into(), items);
    }
    fn mutations(&self) -> Vec<(String, Value)> {
        self.state
            .lock()
            .unwrap()
            .requests
            .iter()
            .filter(|(path, _)| {
                !matches!(
                    path.as_str(),
                    "/file/list" | "/file/getDownloadUrl" | "/payload"
                ) && !path.starts_with("/payload/")
            })
            .cloned()
            .collect()
    }

    fn sync_command(&self, root: &std::path::Path) -> std::process::Command {
        self.sync_command_to(root, "cloud:/")
    }

    fn sync_command_to(&self, root: &std::path::Path, destination: &str) -> std::process::Command {
        let source = root.join("source");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::write(source.join("movie.bin"), b"new content").unwrap();
        let mut command = self.cli(root);
        command.arg("sync").arg(&source).arg(destination);
        command
    }

    fn cli(&self, root: &std::path::Path) -> std::process::Command {
        let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_cloud139"));
        command.arg("--config").arg(self.save_profile(root));
        isolate_cli(&mut command, root);
        command
    }

    fn save_profile(&self, root: &std::path::Path) -> std::path::PathBuf {
        let config = root.join("profile.toml");
        let mut profile = self.config();
        profile.token_expire_time = Some(i64::MAX);
        profile.save_to(&config).unwrap();
        config
    }
}

fn sync_report(output: std::process::Output, status: i32) -> Value {
    let text = String::from_utf8(output.stdout).unwrap();
    let logs = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(status), "{text}\n{logs}");
    assert_eq!(text.lines().count(), 1, "{text}");
    let report: Value = serde_json::from_str(&text).unwrap();
    assert!(report["seconds"].is_number());
    report
}

fn verified_file(id: &str, name: &str, bytes: &[u8], publish_hash: bool) -> Value {
    let mut item = file(id, name);
    item["size"] = json!(bytes.len());
    if publish_hash {
        item["contentHash"] = json!(hash_bytes(bytes));
        item["contentHashAlgorithm"] = json!("SHA-256");
    }
    item
}

#[test]
fn verified_sync_streams_missing_hashes_and_repairs_equal_size_content_before_delete() {
    let server = Server::new();
    let root = tempfile::tempdir().unwrap();
    server.set(
        "/",
        vec![
            verified_file("old", "movie.bin", b"old content", false),
            file("extra", "extra.txt"),
            folder("dir", "extra-dir"),
        ],
    );
    {
        let mut state = server.state.lock().unwrap();
        state.sync_mutations = true;
        state.uploaded_content = Some((b"new content", false));
        state.file_payloads.insert("old".into(), b"old content");
    }
    let report = sync_report(
        server
            .sync_command(root.path())
            .args(["-r", "--verify", "--json", "--delete"])
            .output()
            .unwrap(),
        0,
    );
    assert_eq!(report["objects"], 1);
    assert_eq!(report["uploaded"], 1);
    assert_eq!(report["skipped"], 0);
    assert_eq!(report["deleted"], 1);
    assert_eq!(report["hashMissing"], 1);
    assert_eq!(report["failures"], json!([]));
    assert_eq!(
        report["changes"],
        json!([
            {"path": "extra.txt", "action": "delete"},
            {"path": "movie.bin", "action": "upload"}
        ])
    );
    assert_eq!(report["plan"], json!([]));
    assert_eq!(report["dryRun"], false);

    let state = server.state.lock().unwrap();
    let writes: Vec<_> = state
        .requests
        .iter()
        .filter(|(path, _)| {
            ["/file/create", "/file/update", "/recyclebin/batchTrash"].contains(&path.as_str())
        })
        .map(|(path, body)| (path.as_str(), body))
        .collect();
    assert_eq!(writes.len(), 5);
    assert_eq!(writes[0].0, "/file/create");
    assert_eq!(writes[1].1["fileId"], "old");
    assert_eq!(writes[2].1["name"], "movie.bin");
    assert_eq!(writes[3].1["fileIds"], json!(["old"]));
    assert_eq!(writes[4].1["fileIds"], json!(["extra"]));
    assert!(
        state.children["/"]
            .iter()
            .any(|item| item["fileId"] == "dir")
    );
    let downloads: Vec<_> = state
        .requests
        .iter()
        .filter(|(path, _)| path == "/file/getDownloadUrl")
        .map(|(_, body)| &body["fileId"])
        .collect();
    assert_eq!(downloads.len(), 2);
    assert_eq!(downloads[0], "old");
    assert_eq!(downloads[1], &writes[2].1["fileId"]);
}

#[test]
fn verified_sync_reports_known_and_streamed_content_matches_without_writes() {
    for publish_hash in [true, false] {
        let server = Server::new();
        let root = tempfile::tempdir().unwrap();
        server.set(
            "/",
            vec![verified_file(
                "old",
                "movie.bin",
                b"new content",
                publish_hash,
            )],
        );
        server
            .state
            .lock()
            .unwrap()
            .file_payloads
            .insert("old".into(), b"new content");
        let output = server
            .sync_command(root.path())
            .args(["--verify", "--json"])
            .output()
            .unwrap();
        let report = sync_report(output, 0);
        assert_eq!(report["uploaded"], 0);
        assert_eq!(report["skipped"], 1);
        assert_eq!(report["hashMissing"], usize::from(!publish_hash));
        assert_eq!(report["changes"], json!([]));
        assert!(server.mutations().is_empty());
        let state = server.state.lock().unwrap();
        assert_eq!(
            state
                .requests
                .iter()
                .filter(|(path, _)| path == "/file/list")
                .count(),
            1,
            "校验只列举初始扫描所需的目录"
        );
        assert_eq!(
            state
                .requests
                .iter()
                .filter(|(path, _)| path == "/file/getDownloadUrl")
                .count(),
            usize::from(!publish_hash)
        );
    }
}

#[test]
fn verified_sync_does_not_trust_hash_shaped_filenames_or_delete_on_inconclusive_verification() {
    let server = Server::new();
    let root = tempfile::tempdir().unwrap();
    let actual_name = hash_bytes(b"new content");
    server.set(
        "/",
        vec![
            verified_file("old", &actual_name, b"old content", false),
            file("extra", "extra.txt"),
        ],
    );
    server.state.lock().unwrap().fail_download = Some("old".into());
    let mut command = server.sync_command(root.path());
    let source = root.path().join("source");
    std::fs::rename(source.join("movie.bin"), source.join(&actual_name)).unwrap();
    let report = sync_report(
        command
            .args(["--verify", "--json", "--delete"])
            .output()
            .unwrap(),
        1,
    );
    assert_eq!(report["hashMissing"], 1);
    assert_eq!(report["skipped"], 0);
    assert_eq!(report["deleted"], 0);
    assert_eq!(report["failures"][0]["path"], actual_name);
    assert_eq!(report["failures"][0]["retries"], 0);
    assert!(
        report["failures"][0]["error"]
            .as_str()
            .unwrap()
            .contains("读取内容校验失败")
    );
    assert!(server.mutations().is_empty());
}

#[test]
fn verified_sync_preserves_existing_target_and_extras_when_staged_hash_differs() {
    let server = Server::new();
    let root = tempfile::tempdir().unwrap();
    server.set(
        "/",
        vec![
            verified_file("old", "movie.bin", b"old content", true),
            file("extra", "extra.txt"),
        ],
    );
    {
        let mut state = server.state.lock().unwrap();
        state.sync_mutations = true;
        state.uploaded_content = Some((b"bad content", true));
    }
    let report = sync_report(
        server
            .sync_command(root.path())
            .args(["--verify", "--json", "--delete"])
            .output()
            .unwrap(),
        1,
    );
    assert_eq!(report["uploaded"], 0);
    assert_eq!(report["deleted"], 0);
    assert!(
        report["failures"][0]["error"]
            .as_str()
            .unwrap()
            .contains("SHA-256 不一致")
    );
    assert_eq!(
        server
            .mutations()
            .iter()
            .map(|(path, _)| path.as_str())
            .collect::<Vec<_>>(),
        ["/file/create"]
    );
    let state = server.state.lock().unwrap();
    assert!(
        state.children["/"]
            .iter()
            .any(|item| item["fileId"] == "old" && item["name"] == "movie.bin")
    );
    assert!(
        state.children["/"]
            .iter()
            .any(|item| item["fileId"] == "extra")
    );
}

#[test]
fn verified_sync_rechecks_frozen_delete_ids_before_any_extra_file_is_removed() {
    let server = Server::new();
    let root = tempfile::tempdir().unwrap();
    server.set(
        "/",
        vec![file("another", "another.txt"), file("extra", "extra.txt")],
    );
    {
        let mut state = server.state.lock().unwrap();
        state.sync_mutations = true;
        state.uploaded_content = Some((b"new content", true));
        state.replace_extra_on_promotion = true;
    }
    let report = sync_report(
        server
            .sync_command(root.path())
            .args(["--verify", "--json", "--delete"])
            .output()
            .unwrap(),
        1,
    );
    assert_eq!(report["uploaded"], 1);
    assert_eq!(report["deleted"], 0);
    assert_eq!(report["failures"][0]["path"], "extra.txt");
    assert!(
        server
            .mutations()
            .iter()
            .all(|(path, _)| path != "/recyclebin/batchTrash")
    );
}

#[test]
fn sync_never_replaces_a_file_inside_a_parent_moved_during_upload() {
    for verify in [false, true] {
        let server = Server::new();
        let root = tempfile::tempdir().unwrap();
        server.set("/", vec![folder("dest", "destination")]);
        server.set("dest", vec![file("old", "movie.bin")]);
        {
            let mut state = server.state.lock().unwrap();
            state.sync_mutations = true;
            state.uploaded_content = Some((b"new content", true));
            state.move_parent_on_stage = true;
        }
        let mut command = server.sync_command_to(root.path(), "cloud:/destination");
        if verify {
            command.arg("--verify");
        }
        let output = command.output().unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{stderr}");
        assert!(stderr.contains("无法复核云端目录"), "{stderr}");
        assert_eq!(
            server
                .mutations()
                .iter()
                .map(|(path, _)| path.as_str())
                .collect::<Vec<_>>(),
            ["/file/create"]
        );
        let state = server.state.lock().unwrap();
        assert_eq!(state.children["/"], vec![folder("dest", "moved")]);
        assert_eq!(state.children["dest"][0], file("old", "movie.bin"));
    }
}

#[test]
fn verified_sync_stops_pruning_if_a_later_extra_changes_after_preflight() {
    let server = Server::new();
    let root = tempfile::tempdir().unwrap();
    server.set(
        "/",
        vec![
            verified_file("old", "movie.bin", b"new content", true),
            file("another", "another.txt"),
            file("extra", "extra.txt"),
        ],
    );
    {
        let mut state = server.state.lock().unwrap();
        state.sync_mutations = true;
        state.replace_extra_after_trash = true;
    }
    let report = sync_report(
        server
            .sync_command(root.path())
            .args(["--verify", "--json", "--delete"])
            .output()
            .unwrap(),
        1,
    );
    assert_eq!(report["skipped"], 1);
    assert_eq!(report["deleted"], 1);
    assert_eq!(report["failures"][0]["path"], "extra.txt");
    assert_eq!(
        report["changes"],
        json!([{"path": "another.txt", "action": "delete"}])
    );
    let writes = server.mutations();
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].1["fileIds"], json!(["another"]));
    let state = server.state.lock().unwrap();
    assert!(
        state.children["/"]
            .iter()
            .any(|item| item["fileId"] == "different-extra")
    );
}

#[test]
fn verified_sync_dry_run_has_no_content_reads_and_refuses_to_prune_an_empty_scope() {
    let server = Server::new();
    let root = tempfile::tempdir().unwrap();
    server.set(
        "/",
        vec![
            verified_file("old", "movie.bin", b"old content", false),
            file("extra", "extra.txt"),
        ],
    );
    let report = sync_report(
        server
            .sync_command(root.path())
            .args(["--verify", "--json", "--delete", "-n"])
            .output()
            .unwrap(),
        0,
    );
    assert_eq!(report["objects"], 1);
    assert_eq!(report["uploaded"], 0);
    assert_eq!(report["skipped"], 0);
    assert_eq!(report["deleted"], 1);
    assert_eq!(report["dryRun"], true);
    assert_eq!(report["changes"], json!([]));
    assert_eq!(
        report["plan"],
        json!([
            {"path": "extra.txt", "action": "delete"},
            {"path": "movie.bin", "action": "verify"}
        ])
    );
    assert!(
        server
            .state
            .lock()
            .unwrap()
            .requests
            .iter()
            .all(|(path, _)| path == "/file/list")
    );
    server.state.lock().unwrap().requests.clear();
    let output = server
        .sync_command(root.path())
        .args(["--verify", "--json", "--delete", "--exclude", "movie.bin"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(server.state.lock().unwrap().requests.is_empty());
}

#[test]
fn verified_sync_rejects_corrupt_content_addressed_local_file_and_keeps_remote() {
    let server = Server::new();
    let root = tempfile::tempdir().unwrap();
    let good = b"good";
    let name = hash_bytes(good);
    server.set(
        "/",
        vec![
            verified_file("good-id", &name, good, true),
            file("extra", "extra.txt"),
        ],
    );
    {
        let mut state = server.state.lock().unwrap();
        state.sync_mutations = true;
        state.uploaded_content = Some((b"broken", true));
    }
    let mut command = server.sync_command(root.path());
    let source = root.path().join("source");
    std::fs::remove_file(source.join("movie.bin")).unwrap();
    std::fs::write(source.join(&name), b"broken").unwrap();
    command.args(["--verify", "--json", "--delete"]);
    let report = sync_report(command.output().unwrap(), 1);
    assert_eq!(report["uploaded"], 0);
    assert_eq!(report["deleted"], 0);
    assert_eq!(report["failures"][0]["path"], name);
    assert!(
        server.mutations().is_empty(),
        "本地内容身份检查必须在创建远端对象之前完成，不能有 create/trash：{:?}",
        server.mutations()
    );
    let state = server.state.lock().unwrap();
    assert!(
        state.children["/"]
            .iter()
            .any(|item| item["fileId"] == "good-id" && item["size"] == json!(good.len())),
        "远端完好对象必须保留"
    );
    assert!(
        state.children["/"]
            .iter()
            .any(|item| item["fileId"] == "extra"),
        "删除闸门必须关闭，多余文件不能删"
    );
}

#[test]
fn verified_sync_rejects_matching_content_that_disagrees_with_its_name() {
    let server = Server::new();
    let root = tempfile::tempdir().unwrap();
    let name = hash_bytes(b"good");
    let bytes = b"bad!";
    server.set("/", vec![verified_file("old", &name, bytes, true)]);
    let source = root.path().join("source");
    std::fs::create_dir(&source).unwrap();
    std::fs::write(source.join(&name), bytes).unwrap();
    let report = sync_report(
        server
            .cli(root.path())
            .arg("sync")
            .arg(&source)
            .args(["cloud:/", "--verify", "--json"])
            .output()
            .unwrap(),
        1,
    );
    assert_eq!(report["skipped"], 0);
    assert_eq!(report["failures"][0]["path"], name);
    assert!(
        report["failures"][0]["error"]
            .as_str()
            .unwrap()
            .contains("与文件名不一致")
    );
    assert!(server.mutations().is_empty());
}

#[test]
fn plain_sync_accepts_ordinary_hex_filename_without_verify() {
    let server = Server::new();
    let root = tempfile::tempdir().unwrap();
    server.set("/", vec![]);
    {
        let mut state = server.state.lock().unwrap();
        state.sync_mutations = true;
    }
    let mut command = server.sync_command(root.path());
    let source = root.path().join("source");
    std::fs::remove_file(source.join("movie.bin")).unwrap();
    let name = "a".repeat(64);
    std::fs::write(source.join(&name), b"x").unwrap();
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let state = server.state.lock().unwrap();
    assert!(
        state.children["/"].iter().any(|item| item["name"] == name),
        "普通同步必须仍能发布 64-hex 文件名"
    );
}

#[test]
fn verified_sync_never_trashes_a_reused_receipt_id() {
    for corrupt in [true, false] {
        let server = Server::new();
        let root = tempfile::tempdir().unwrap();
        let name = hash_bytes(b"good");
        let mut old = verified_file("good-id", &name, b"good", true);
        old.as_object_mut().unwrap().remove("size");
        server.set("/", vec![old, file("extra", "extra.txt")]);
        {
            let mut state = server.state.lock().unwrap();
            state.sync_mutations = true;
            state.reuse_existing_upload_id = true;
            state.file_payloads.insert("good-id".to_owned(), b"good");
        }
        let mut command = server.sync_command(root.path());
        let source = root.path().join("source");
        std::fs::remove_file(source.join("movie.bin")).unwrap();
        let content = if corrupt {
            &b"broken"[..]
        } else {
            &b"good"[..]
        };
        std::fs::write(source.join(&name), content).unwrap();
        let report = sync_report(
            command
                .args(["--verify", "--json", "--delete"])
                .output()
                .unwrap(),
            1,
        );
        assert_eq!(report["uploaded"], 0);
        assert_eq!(report["deleted"], 0);
        assert_eq!(report["failures"].as_array().unwrap().len(), 1);
        let writes = server.mutations();
        if corrupt {
            assert!(writes.is_empty());
        } else {
            assert_eq!(writes.len(), 1);
            assert_eq!(writes[0].0, "/file/create");
            assert!(
                report["failures"][0]["error"]
                    .as_str()
                    .unwrap()
                    .contains("临时文件 ID 已存在")
            );
        }
        let state = server.state.lock().unwrap();
        assert!(
            state.children["/"]
                .iter()
                .any(|item| item["fileId"] == "good-id" && item["name"] == name)
        );
        assert!(
            state.children["/"]
                .iter()
                .any(|item| item["fileId"] == "extra")
        );
    }
}

fn rejects_content_addressed_source_changed_during_upload(existing: bool) {
    let server = Server::new();
    let root = tempfile::tempdir().unwrap();
    let name = hash_bytes(b"good");
    let mut items = vec![file("extra", "extra.txt")];
    if existing {
        let mut old = verified_file("good-id", &name, b"good", true);
        // 省略已有文件的大小字段，使内容正确的文件仍进入修复流程。
        old.as_object_mut().unwrap().remove("size");
        items.push(old);
    }
    server.set("/", items);
    let mut command = server.sync_command(root.path());
    let source = root.path().join("source");
    std::fs::remove_file(source.join("movie.bin")).unwrap();
    let path = source.join(&name);
    std::fs::write(&path, b"good").unwrap();
    {
        let mut state = server.state.lock().unwrap();
        state.sync_mutations = true;
        state.multipart_content = Some(b"bad!");
        state.file_payloads.insert("good-id".to_owned(), b"good");
        // 第二次列目录发生在前置校验之后、计算实际上传内容的哈希之前。
        state.mutate_source_on_list = Some((2, path.clone(), b"bad!"));
    }
    let report = sync_report(
        command
            .args(["--verify", "--json", "--delete"])
            .output()
            .unwrap(),
        1,
    );
    assert_eq!(std::fs::read(&path).unwrap(), b"bad!");
    assert_eq!(report["uploaded"], 0);
    assert_eq!(report["deleted"], 0);
    assert_eq!(report["failures"].as_array().unwrap().len(), 1);
    assert!(
        report["failures"][0]["error"]
            .as_str()
            .unwrap()
            .contains("上传内容哈希")
    );
    assert!(
        report["failures"][0]["error"]
            .as_str()
            .unwrap()
            .contains(".cloud139-upload-")
    );
    let state = server.state.lock().unwrap();
    let create = state
        .requests
        .iter()
        .find(|(p, _)| p == "/file/create")
        .unwrap();
    assert_eq!(create.1["contentHash"], hash_bytes(b"bad!"));
    assert!(
        state
            .requests
            .iter()
            .any(|(p, body)| p.starts_with("/part/") && body == "bad!")
    );
    assert!(state.requests.iter().any(|(p, _)| p == "/file/complete"));
    assert!(
        state
            .requests
            .iter()
            .all(|(p, _)| p != "/file/update" && p != "/recyclebin/batchTrash")
    );
    assert!(
        state.children["/"]
            .iter()
            .any(|item| item["fileId"] == "extra")
    );
    assert!(state.children["/"].iter().any(|item| {
        item["name"]
            .as_str()
            .unwrap()
            .starts_with(".cloud139-upload-")
            && item["contentHash"] == hash_bytes(b"bad!")
    }));
    if existing {
        assert!(
            state.children["/"]
                .iter()
                .any(|item| item["fileId"] == "good-id" && item["name"] == name)
        );
    } else {
        assert!(state.children["/"].iter().all(|item| item["name"] != name));
    }
}

#[test]
fn verified_sync_rejects_changed_content_addressed_new_upload() {
    rejects_content_addressed_source_changed_during_upload(false);
}

#[test]
fn verified_sync_rejects_changed_content_addressed_replacement() {
    rejects_content_addressed_source_changed_during_upload(true);
}

#[test]
fn verified_sync_publishes_lowercase_uppercase_and_ordinary_names() {
    for name in [
        hash_bytes(b"good"),
        hash_bytes(b"good").to_uppercase(),
        "ordinary.txt".to_owned(),
    ] {
        for existing in [false, true] {
            let server = Server::new();
            let root = tempfile::tempdir().unwrap();
            let items = if existing {
                vec![verified_file("old", &name, b"old!", true)]
            } else {
                vec![]
            };
            server.set("/", items);
            {
                let mut state = server.state.lock().unwrap();
                state.sync_mutations = true;
                state.multipart_content = Some(b"good");
            }
            let mut command = server.sync_command(root.path());
            let source = root.path().join("source");
            std::fs::remove_file(source.join("movie.bin")).unwrap();
            std::fs::write(source.join(&name), b"good").unwrap();
            let report = sync_report(command.args(["--verify", "--json"]).output().unwrap(), 0);
            assert_eq!(report["uploaded"], 1);
            assert_eq!(report["failures"], json!([]));
            let state = server.state.lock().unwrap();
            assert!(
                state
                    .requests
                    .iter()
                    .any(|(p, body)| p.starts_with("/part/") && body == "good")
            );
            assert!(
                state.children["/"]
                    .iter()
                    .any(|item| item["name"] == name && item["contentHash"] == hash_bytes(b"good"))
            );
            assert!(
                state.children["/"]
                    .iter()
                    .all(|item| item["fileId"] != "old")
            );
        }
    }
}

#[test]
fn recursive_sync_download_reuses_scanned_file_ids_and_validates_names() {
    let server = Server::new();
    let root = tempfile::tempdir().unwrap();
    server.set("/", vec![folder("backup-id", "backup")]);
    server.set("backup-id", vec![folder("child-id", "child")]);
    server.set(
        "child-id",
        vec![
            file("first-id", "first.bin"),
            file("second-id", "second.bin"),
        ],
    );
    let destination = root.path().join("download");
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_cloud139"));
    command
        .arg("--config")
        .arg(server.save_profile(root.path()))
        .args(["sync", "cloud:/backup"])
        .arg(&destination)
        .arg("-r");
    isolate_cli(&mut command, root.path());
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    for name in ["first.bin", "second.bin"] {
        assert_eq!(
            std::fs::read(destination.join("child").join(name)).unwrap(),
            b"data"
        );
    }
    {
        let mut state = server.state.lock().unwrap();
        let lists: Vec<_> = state
            .requests
            .iter()
            .filter(|(path, _)| path == "/file/list")
            .map(|(_, body)| body["parentFileId"].as_str().unwrap())
            .collect();
        assert_eq!(lists, ["/", "backup-id", "child-id"]);
        state.requests.clear();
    }
    server.set("child-id", vec![file("unsafe", "..")]);
    let output = command.output().unwrap();
    assert_eq!(
        output.status.code(),
        Some(2),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        server
            .state
            .lock()
            .unwrap()
            .requests
            .iter()
            .all(|(path, _)| path == "/file/list")
    );
}

#[test]
fn sync_json_reports_planned_and_completed_downloads_and_deletions() {
    let server = Server::new();
    let root = tempfile::tempdir().unwrap();
    let mut unchanged = file("unchanged", "same.bin");
    unchanged["lastModified"] = json!("2024-01-01T00:00:00Z");
    server.set("/", vec![file("download", "new.bin"), unchanged]);
    let destination = root.path().join("destination");
    std::fs::create_dir_all(destination.join("empty")).unwrap();
    std::fs::write(destination.join("old.bin"), b"old").unwrap();
    let same = destination.join("same.bin");
    std::fs::write(&same, b"data").unwrap();
    filetime::set_file_mtime(&same, filetime::FileTime::from_unix_time(1_704_067_200, 0)).unwrap();
    let changes = json!([
        {"path": "new.bin", "action": "download"},
        {"path": "old.bin", "action": "delete"}
    ]);
    for dry_run in [true, false] {
        let mut command = server.cli(root.path());
        command.args(["sync", "cloud:/", "destination", "-r", "--delete", "--json"]);
        if dry_run {
            command.arg("-n");
        }
        let report = sync_report(command.output().unwrap(), 0);
        assert_eq!(report["source"], "cloud:/");
        assert_eq!(report["destination"], "destination");
        assert_eq!(report["objects"], 2);
        assert_eq!(report["downloaded"], 1);
        assert_eq!(report["deleted"], 1);
        assert_eq!(report["skipped"], 1);
        assert_eq!(report["failures"], json!([]));
        assert_eq!(report["dryRun"], dry_run);
        if dry_run {
            assert_eq!(report["plan"], changes);
            assert_eq!(report["changes"], json!([]));
            assert!(!destination.join("new.bin").exists());
            assert!(destination.join("old.bin").is_file());
        } else {
            assert_eq!(report["plan"], json!([]));
            assert_eq!(report["changes"], changes);
            assert_eq!(std::fs::read(destination.join("new.bin")).unwrap(), b"data");
            assert!(!destination.join("old.bin").exists());
            assert!(!destination.join("empty").exists());
        }
    }
}

#[test]
fn sync_json_reports_successful_downloads_when_a_peer_fails_and_pruning_is_blocked() {
    let server = Server::new();
    let root = tempfile::tempdir().unwrap();
    server.set(
        "/",
        vec![file("success", "good.bin"), file("failure", "bad.bin")],
    );
    server.state.lock().unwrap().fail_download = Some("failure".into());
    let destination = root.path().join("destination");
    std::fs::create_dir(&destination).unwrap();
    std::fs::write(destination.join("extra.bin"), b"keep").unwrap();
    let report = sync_report(
        server
            .cli(root.path())
            .args(["sync", "cloud:/", "destination", "--delete", "--json"])
            .output()
            .unwrap(),
        1,
    );
    assert_eq!(
        report["changes"],
        json!([{"path": "good.bin", "action": "download"}])
    );
    assert_eq!(report["failures"][0]["path"], "bad.bin");
    assert_eq!(report["downloaded"], 1);
    assert_eq!(report["deleted"], 0);
    assert_eq!(
        std::fs::read(destination.join("extra.bin")).unwrap(),
        b"keep"
    );
}

fn isolate_cli(command: &mut std::process::Command, root: &std::path::Path) {
    command
        .current_dir(root)
        .env("HOME", root)
        .env("XDG_CONFIG_HOME", root)
        .env("USERPROFILE", root);
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            worker.join().unwrap();
        }
    }
}
fn read_request(stream: &mut TcpStream) -> Option<(String, Value)> {
    let mut data = Vec::new();
    let mut buffer = [0; 4096];
    let end = loop {
        let n = stream.read(&mut buffer).ok()?;
        if n == 0 {
            return None;
        }
        data.extend_from_slice(&buffer[..n]);
        if let Some(index) = data.windows(4).position(|w| w == b"\r\n\r\n") {
            break index + 4;
        }
    };
    let header = String::from_utf8_lossy(&data[..end]);
    let path = header.lines().next()?.split_whitespace().nth(1)?.to_owned();
    let length = header
        .lines()
        .filter_map(|l| l.split_once(':'))
        .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, v)| v.trim().parse::<usize>().ok())
        .unwrap_or(0);
    while data.len() < end + length {
        let n = stream.read(&mut buffer).ok()?;
        if n == 0 {
            return None;
        }
        data.extend_from_slice(&buffer[..n]);
    }
    let body = if length == 0 {
        Value::Null
    } else if path.starts_with("/part/") {
        json!(std::str::from_utf8(&data[end..end + length]).ok()?)
    } else {
        serde_json::from_slice(&data[end..end + length]).ok()?
    };
    Some((path, body))
}
fn folder(id: &str, name: &str) -> Value {
    json!({"fileId":id,"name":name,"type":"folder"})
}
fn file(id: &str, name: &str) -> Value {
    json!({"fileId":id,"name":name,"type":"file","size":4})
}

fn hash_bytes(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(bytes))
}

#[test]
fn removed_options_fail_before_config_changes() {
    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("profile.toml");
    let original = br#"account = "test"
authorization = "test"
storage_type = "personal_new"
"#;
    std::fs::write(&config, original).unwrap();

    for (args, flag) in [
        (
            &["login", "--token", "invalid", "--storage-type", "group"][..],
            "--storage-type",
        ),
        (
            &["login", "--token", "invalid", "--cloud-id", "test"],
            "--cloud-id",
        ),
        (
            &["login", "--token", "invalid", "--root-folder-id", "test"],
            "--root-folder-id",
        ),
        (&["mkdir", "--force", "/folder"], "--force"),
        (&["mkdir", "-f", "/folder"], "-f"),
        (&["rm", "--yes", "--permanent", "/file"], "--permanent"),
        (&["rm", "--yes", "-p", "/file"], "-p"),
    ] {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_cloud139"))
            .current_dir(temp.path())
            .arg("--config")
            .arg(&config)
            .args(args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(flag), "{stderr}");
        assert_eq!(std::fs::read(&config).unwrap(), original);
    }
}

#[tokio::test]
async fn recursive_download_rejects_local_case_or_unicode_collisions() {
    let server = Server::new();
    server.set("/", vec![folder("dir", "folder")]);
    let tmp = tempfile::tempdir().unwrap();
    for (index, (first, second)) in [("A.txt", "a.txt"), ("caf\u{e9}.txt", "cafe\u{301}.txt")]
        .into_iter()
        .enumerate()
    {
        let volume_probe = tempfile::tempdir_in(tmp.path()).unwrap();
        std::fs::write(volume_probe.path().join(first), b"probe").unwrap();
        let aliases = volume_probe.path().join(second).exists();
        server.set("dir", vec![file("one", first), file("two", second)]);
        let destination = tmp.path().join(format!("download-{index}"));
        let result = services::download::download(
            &server.config(),
            "/folder",
            destination.to_str().unwrap(),
            None,
            true,
        )
        .await;
        if aliases {
            assert!(
                result
                    .unwrap_err()
                    .to_string()
                    .contains("映射到同一本地路径")
            );
            assert!(!destination.exists());
        } else {
            result.unwrap();
            assert_eq!(std::fs::read_dir(&destination).unwrap().count(), 2);
        }
    }
}

#[tokio::test]
async fn personal_trash_reports_item_failure_even_if_batch_succeeded() {
    let server = Server::new();
    server.set("/", vec![file("one", "report.txt")]);
    server.state.lock().unwrap().fail_trash_item = true;
    let error = services::delete::delete(&server.config(), "/report.txt", false)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("删除失败"));
    assert_eq!(server.mutations().len(), 1);
}

#[test]
fn normalizes_root_aliases_and_checks_names() {
    for path in ["", "/", "//", "/./", "/a/..", "a/../.", "/../../"] {
        assert_eq!(remote_path::normalize(path).unwrap(), "/");
        assert!(matches!(
            remote_path::non_root(path),
            Err(ClientError::CannotOperateOnRoot)
        ));
    }
    assert_eq!(remote_path::normalize("//a/./b/../c/").unwrap(), "/a/c");
    for name in [
        "",
        ".",
        "..",
        "../escape",
        "/absolute",
        "a\\b",
        "C:escape",
        "x\n",
        "NUL",
        "COM1.txt",
        "x.",
        "x ",
    ] {
        assert!(remote_path::validate_name(name).is_err(), "{name:?}");
    }
}

#[tokio::test]
async fn root_mutations_are_rejected_for_personal_storage_without_network() {
    {
        let config = Config::default();
        for root in ["/", "//./", "/a/..", ""] {
            assert!(
                services::delete::delete(&config, root, true)
                    .await
                    .unwrap_err()
                    .to_string()
                    .contains("根目录")
            );
            assert!(matches!(
                services::rename::rename(&config, root, "safe", false).await,
                Err(ClientError::CannotOperateOnRoot)
            ));
            assert!(matches!(
                services::directory::cp(&config, &[root.into()], "/dst", true).await,
                Err(ClientError::CannotOperateOnRoot)
            ));
            assert!(matches!(
                services::directory::mv(&config, &[root.into()], "/dst", true).await,
                Err(ClientError::CannotOperateOnRoot)
            ));
        }
    }
}

#[tokio::test]
async fn rejects_self_and_descendant_with_force_before_requests() {
    let config = Config::default();
    for target in ["/a", "/a/sub", "/a/./sub/child", "/x/../a/sub"] {
        assert!(
            services::directory::cp(&config, &["/a".into()], target, true)
                .await
                .unwrap_err()
                .to_string()
                .contains("自身或子目录")
        );
        assert!(
            services::directory::mv(&config, &["/a".into()], target, true)
                .await
                .unwrap_err()
                .to_string()
                .contains("自身或子目录")
        );
    }
}

#[tokio::test]
async fn mkdir_parents_is_idempotent_and_rejects_files_without_rename() {
    let server = Server::new();
    let config = server.config();
    services::mkdir::mkdir(&config, "/a/b/empty", services::mkdir::MkdirMode::Parents)
        .await
        .unwrap();
    assert_eq!(server.mutations().len(), 3);
    services::mkdir::mkdir(&config, "/a/b/empty/", services::mkdir::MkdirMode::Parents)
        .await
        .unwrap();
    assert_eq!(server.mutations().len(), 3);
    server.set("/", vec![file("f", "occupied")]);
    assert!(
        services::mkdir::mkdir(
            &config,
            "/occupied/child",
            services::mkdir::MkdirMode::Parents
        )
        .await
        .is_err()
    );
    assert_eq!(server.mutations().len(), 3);
    assert!(
        server
            .mutations()
            .iter()
            .all(|(_, body)| body.get("fileRenameMode").is_none())
    );
}

#[tokio::test]
async fn recursive_listing_collects_all_pages_empty_dirs_and_detects_cycles() {
    let server = Server::new();
    server.state.lock().unwrap().page_size = 1;
    server.set("/", vec![folder("d", "dir"), file("f", "last.bin")]);
    server.set("d", vec![folder("e", "empty"), file("n", "nested.bin")]);
    let result = services::list::list_recursive(&server.config(), "/")
        .await
        .unwrap();
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].items.len(), 2);
    assert!(
        result
            .iter()
            .any(|r| r.path == "/dir/empty" && r.items.is_empty())
    );
    server.state.lock().unwrap().cycle = true;
    assert!(
        services::list::list_recursive(&server.config(), "/")
            .await
            .unwrap_err()
            .to_string()
            .contains("分页游标重复")
    );
    server.state.lock().unwrap().cycle = false;
    server.set("d", vec![folder("d", "loop")]);
    assert!(
        services::list::list_recursive(&server.config(), "/")
            .await
            .unwrap_err()
            .to_string()
            .contains("目录循环")
    );
}

#[tokio::test]
async fn ordinary_ls_rejects_cursor_cycles() {
    let server = Server::new();
    server.state.lock().unwrap().cycle = true;
    let args = cloud139::services::list::ListOptions {
        path: "/",
        page: 1,
        page_size: 100,
    };
    assert!(
        services::list::list(&server.config(), args)
            .await
            .unwrap_err()
            .to_string()
            .contains("分页游标重复")
    );
}

#[tokio::test]
async fn recursive_download_requires_flag_preserves_empty_and_file_contents() {
    let server = Server::new();
    server.set("/", vec![folder("d", "dir")]);
    server.set("d", vec![folder("e", "empty"), file("f", "data.bin")]);
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("out");
    let local = out.to_str().unwrap();
    assert!(
        services::download::download(&server.config(), "/dir", local, None, false)
            .await
            .is_err()
    );
    assert!(!out.exists());
    services::download::download(&server.config(), "/dir", local, None, true)
        .await
        .unwrap();
    assert!(out.join("empty").is_dir());
    assert_eq!(std::fs::read(out.join("data.bin")).unwrap(), b"data");
}

#[tokio::test]
async fn recursive_download_validates_listed_size_without_content_length() {
    let server = Server::new();
    server.set("/", vec![folder("d", "dir")]);
    let tmp = tempfile::tempdir().unwrap();
    let destination = tmp.path().join("data.bin");
    let local = tmp.path().to_str().unwrap();

    for (listed_size, payload, valid) in [
        (Some(7), b"abc".as_slice(), false),
        (Some(7), b"abcdefg".as_slice(), true),
        (Some(0), b"abc".as_slice(), false),
        (Some(0), b"".as_slice(), true),
        (None, b"abc".as_slice(), true),
    ] {
        let mut item = file("f", "data.bin");
        item["size"] = json!(listed_size);
        server.set("d", vec![item]);
        server.state.lock().unwrap().close_delimited_payload = Some(payload);
        std::fs::write(&destination, b"keep original").unwrap();
        let result =
            services::download::download(&server.config(), "/dir", local, None, true).await;
        if valid {
            result.unwrap();
            assert_eq!(std::fs::read(&destination).unwrap(), payload);
        } else {
            let message = result.expect_err("mismatched body must fail").to_string();
            assert!(message.contains("下载大小不匹配"), "{message}");
            assert_eq!(std::fs::read(&destination).unwrap(), b"keep original");
        }
        assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 1);
    }
}

#[tokio::test]
async fn malicious_names_and_local_kind_conflicts_fail_before_writes() {
    let server = Server::new();
    server.set("/", vec![folder("d", "dir")]);
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("out");
    for name in ["../escape", "/escape", "C:escape", "a\\b", "."] {
        server.set("d", vec![file("f", name)]);
        assert!(
            services::download::download(
                &server.config(),
                "/dir",
                out.to_str().unwrap(),
                None,
                true
            )
            .await
            .is_err()
        );
        assert!(!out.exists());
    }
    server.set("d", vec![folder("e", "empty")]);
    std::fs::create_dir(&out).unwrap();
    std::fs::write(out.join("empty"), b"keep").unwrap();
    assert!(
        services::download::download(&server.config(), "/dir", out.to_str().unwrap(), None, true)
            .await
            .is_err()
    );
    assert_eq!(std::fs::read(out.join("empty")).unwrap(), b"keep");
}

#[tokio::test]
async fn pc_sync_preserves_old_directory_on_rejection_and_replaces_after_upload() {
    let server = Server::new();
    let tmp = tempfile::tempdir().unwrap();
    server.set("/", vec![folder("old", "movie.bin")]);
    server.set("old", vec![file("keep", "keep.txt")]);
    {
        let mut state = server.state.lock().unwrap();
        state.fail_upload = true;
        state.sync_mutations = true;
    }
    for (dry_run, fail_upload) in [(true, true), (false, true), (false, false)] {
        {
            let mut state = server.state.lock().unwrap();
            state.requests.clear();
            state.fail_upload = fail_upload;
            state.staged_visibility_delay = if !dry_run && !fail_upload { 2 } else { 0 };
        }
        let mut command = server.sync_command(tmp.path());
        command.args(["-r", "--delete", "--json"]);
        if dry_run {
            command.arg("-n");
        }
        let output = command.output().unwrap();
        let text = String::from_utf8_lossy(&output.stderr).into_owned();
        let report = sync_report(output, i32::from(!dry_run && fail_upload));
        let replacement = json!([
            {"path": "movie.bin", "action": "upload"},
            {"path": "movie.bin/keep.txt", "action": "delete"}
        ]);
        let state = server.state.lock().unwrap();
        let old_present = state.children["/"]
            .iter()
            .any(|item| item["fileId"] == "old" && item["name"] == "movie.bin");
        if dry_run || fail_upload {
            assert!(old_present);
            assert!(
                !state
                    .requests
                    .iter()
                    .any(|(p, _)| p == "/file/update" || p == "/recyclebin/batchTrash")
            );
            if dry_run {
                assert_eq!(report["plan"], replacement);
                assert_eq!(report["changes"], json!([]));
            } else {
                assert_eq!(report["changes"], json!([]));
                assert!(text.contains("04010319"), "{text}");
            }
        } else {
            assert_eq!(report["changes"], replacement);
            assert!(!old_present);
            assert_eq!(state.staged_visibility_delay, 0);
            assert!(
                state.children["/"]
                    .iter()
                    .any(|item| item["name"] == "movie.bin" && item["type"] == "file")
            );
            let writes: Vec<_> = state
                .requests
                .iter()
                .filter(|(p, _)| p != "/file/list")
                .collect();
            assert_eq!(writes.len(), 4);
            assert_eq!(writes[0].0, "/file/create");
            assert_eq!(writes[1].0, "/file/update");
            assert_eq!(writes[1].1["fileId"], "old");
            assert_eq!(writes[2].0, "/file/update");
            assert_eq!(writes[2].1["name"], "movie.bin");
            assert_eq!(writes[3].0, "/recyclebin/batchTrash");
            assert!(!text.contains(".cloud139-upload-"), "{text}");
        }
    }
}

#[test]
fn sync_json_reports_a_published_file_even_if_backup_cleanup_fails() {
    for verify in [false, true] {
        let server = Server::new();
        let root = tempfile::tempdir().unwrap();
        server.set("/", vec![verified_file("old", "movie.bin", b"old", true)]);
        {
            let mut state = server.state.lock().unwrap();
            state.sync_mutations = true;
            state.fail_trash_item = true;
            state.uploaded_content = Some((b"new content", true));
        }
        let mut command = server.sync_command(root.path());
        command.arg("--json");
        if verify {
            command.arg("--verify");
        }
        let report = sync_report(command.output().unwrap(), 1);
        assert_eq!(
            report["changes"],
            json!([{"path": "movie.bin", "action": "upload"}])
        );
        assert_eq!(report["failures"][0]["path"], "movie.bin");
        assert!(
            report["failures"][0]["error"]
                .as_str()
                .unwrap()
                .contains("回收站")
        );
    }
}

#[test]
fn pc_sync_delete_retry_preserves_recovery_copies_and_their_ancestors() {
    let server = Server::new();
    let tmp = tempfile::tempdir().unwrap();
    server.set("/", vec![folder("old", "movie.bin")]);
    server.set("old", vec![file("keep", "keep.txt")]);
    {
        let mut state = server.state.lock().unwrap();
        state.sync_mutations = true;
        state.fail_rename_to = Some("movie.bin".into());
    }
    let run_sync = || {
        server
            .sync_command(tmp.path())
            .args(["-r", "--delete", "-j", "1"])
            .output()
            .unwrap()
    };
    let first = run_sync();
    assert_eq!(first.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&first.stderr).contains("injected rename failure"));
    let recovery_items = {
        let mut state = server.state.lock().unwrap();
        let root = &state.children["/"];
        assert_eq!(root.len(), 2);
        assert!(root.iter().any(|item| {
            item["fileId"] == "old"
                && item["name"]
                    .as_str()
                    .unwrap()
                    .starts_with(".cloud139-backup-")
        }));
        assert!(root.iter().any(|item| {
            item["name"]
                .as_str()
                .unwrap()
                .starts_with(".cloud139-upload-")
        }));
        let recovery_items = root.clone();
        state.requests.clear();
        state.fail_upload = true;
        recovery_items
    };
    let mut root = recovery_items.clone();
    root.extend([folder("parent", "orphan"), file("extra", "extra.txt")]);
    server.set("/", root);
    server.set(
        "parent",
        vec![folder("nested-backup", ".cloud139-backup-example")],
    );
    server.set("nested-backup", vec![file("nested-child", "keep.txt")]);

    let retry = run_sync();
    assert_eq!(retry.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&retry.stderr).contains("04010319"));
    let state = server.state.lock().unwrap();
    let mut expected = recovery_items;
    expected.extend([folder("parent", "orphan"), file("extra", "extra.txt")]);
    assert_eq!(state.children["/"], expected);
    assert_eq!(state.children["old"], vec![file("keep", "keep.txt")]);
    assert_eq!(
        state.children["nested-backup"],
        vec![file("nested-child", "keep.txt")]
    );
    let trash: Vec<_> = state
        .requests
        .iter()
        .filter(|(path, _)| path == "/recyclebin/batchTrash")
        .map(|(_, body)| &body["fileIds"])
        .collect();
    assert!(trash.is_empty());
}

#[test]
fn pc_sync_refuses_to_replace_cloud_directories_outside_the_scope() {
    let server = Server::new();
    let tmp = tempfile::tempdir().unwrap();
    server.set("/", vec![folder("old", "movie.bin")]);
    server.set(
        "old",
        vec![folder("nested", "nested"), file("other", "other.txt")],
    );
    server.set("nested", vec![file("keep", "keep.txt")]);
    server.state.lock().unwrap().sync_mutations = true;

    for scope in [&[][..], &["-r", "--exclude", "movie.bin/**/keep.txt"][..]] {
        for dry_run in [true, false] {
            let mut command = server.sync_command(tmp.path());
            command.args(scope).arg("--delete");
            if dry_run {
                command.arg("-n");
            }
            let output = command.output().unwrap();
            let text = format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(output.status.code(), Some(i32::from(!dry_run)), "{text}");
            assert!(text.contains("目标同名目录未完整纳入同步范围"), "{text}");
            assert!(server.mutations().is_empty(), "{text}");
        }
    }
    let state = server.state.lock().unwrap();
    assert_eq!(state.children["/"], vec![folder("old", "movie.bin")]);
    assert_eq!(state.children["old"].len(), 2);
    assert_eq!(state.children["nested"], vec![file("keep", "keep.txt")]);
}

#[test]
fn pc_sync_refuses_staged_files_without_a_matching_creation_id() {
    for missing in [true, false] {
        let server = Server::new();
        let tmp = tempfile::tempdir().unwrap();
        server.set("/", vec![file("old", "movie.bin")]);
        {
            let mut state = server.state.lock().unwrap();
            state.sync_mutations = true;
            state.omit_staged_id = missing;
            state.staged_id_mismatch = !missing;
        }
        let output = server.sync_command(tmp.path()).output().unwrap();
        let text = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{text}");
        assert!(
            text.contains(if missing {
                "未返回新文件 ID"
            } else {
                "身份"
            }),
            "{text}"
        );
        let writes = server.mutations();
        assert_eq!(writes.len(), 1, "{writes:?}");
        assert_eq!(writes[0].0, "/file/create");
        assert_eq!(
            server.state.lock().unwrap().children["/"][0],
            file("old", "movie.bin")
        );
    }
}

#[tokio::test]
async fn upload_maps_directory_or_contents_keeps_empty_and_never_skips_same_size() {
    let server = Server::new();
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("source");
    std::fs::create_dir_all(source.join("empty")).unwrap();
    std::fs::write(source.join("file.bin"), b"new!").unwrap();
    services::upload::upload_directory(&server.config(), &source, "/missing/target", false, false)
        .await
        .unwrap();
    let names: Vec<_> = server
        .mutations()
        .iter()
        .map(|(_, v)| v["name"].as_str().unwrap().to_owned())
        .collect();
    assert!(
        names.contains(&"source".into())
            && names.contains(&"empty".into())
            && names.contains(&"file.bin".into())
    );
    let uploads = server
        .mutations()
        .iter()
        .filter(|(_, b)| b["type"] == "file")
        .count();
    services::upload::upload_directory(&server.config(), &source, "/missing/target", false, true)
        .await
        .unwrap();
    assert_eq!(
        server
            .mutations()
            .iter()
            .filter(|(_, b)| b["type"] == "file")
            .count(),
        uploads + 1
    );
    let second = Server::new();
    services::upload::upload_directory(&second.config(), &source, "/target", true, false)
        .await
        .unwrap();
    assert!(
        !second
            .mutations()
            .iter()
            .any(|(_, b)| b["name"] == "source")
    );
}

#[tokio::test]
async fn upload_and_batch_errors_propagate_partial_failure() {
    let server = Server::new();
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("a.bin"), b"data").unwrap();
    std::fs::write(tmp.path().join("z.bin"), b"fail").unwrap();
    std::fs::write(tmp.path().join("zz.bin"), b"after failure").unwrap();
    server.state.lock().unwrap().fail_name = Some("z.bin".into());
    let error =
        services::upload::upload_directory(&server.config(), tmp.path(), "/target", true, false)
            .await
            .unwrap_err();
    assert!(error.to_string().contains("此前条目可能已完成"));
    let uploads = server.mutations();
    assert_eq!(
        uploads.iter().filter(|(_, b)| b["name"] == "a.bin").count(),
        1
    );
    assert_eq!(
        uploads.iter().filter(|(_, b)| b["name"] == "z.bin").count(),
        1
    );
    assert_eq!(
        uploads
            .iter()
            .filter(|(_, b)| b["name"] == "zz.bin")
            .count(),
        1
    );
    server.set("/", vec![folder("s", "src"), folder("d", "dest")]);
    server.state.lock().unwrap().fail_batch = true;
    assert!(
        services::directory::cp(&server.config(), &["/src".into()], "/dest", false)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn recursive_upload_retries_the_failed_page_without_replaying_writes() {
    let server = Server::new();
    server.set("/", vec![folder("outer", "outer")]);
    server.set("outer", vec![folder("target", "target")]);
    let mut existing: Vec<_> = (0..101)
        .map(|index| {
            file(
                &format!("existing-{index}"),
                &format!("existing-{index}.txt"),
            )
        })
        .collect();
    existing.push(folder("blocker", "z.txt"));
    server.set("target", existing);
    server.state.lock().unwrap().drop_list_cursor_once = Some("100".into());

    let source = tempfile::tempdir().unwrap();
    std::fs::write(source.path().join("a.txt"), b"first").unwrap();
    std::fs::write(source.path().join("b.txt"), b"second").unwrap();
    std::fs::write(source.path().join("z.txt"), b"blocked").unwrap();
    let error = services::upload::upload_directory(
        &server.config(),
        source.path(),
        "/outer/target",
        true,
        true,
    )
    .await
    .unwrap_err();
    let message = error.to_string();
    assert!(message.contains("上传 z.txt 失败"), "{message}");
    assert!(message.contains("云端同名目录阻止上传文件"), "{message}");

    let state = server.state.lock().unwrap();
    let mut target_cursors = Vec::new();
    let mut uploads = Vec::new();
    let mut ancestor_lookups = 0;
    for (path, body) in &state.requests {
        match path.as_str() {
            "/file/list" if body["parentFileId"] == "target" => {
                target_cursors.push(body["pageInfo"]["pageCursor"].as_str().unwrap());
            }
            "/file/list" => ancestor_lookups += 1,
            "/file/create" => {
                assert_eq!(body["parentFileId"], "target");
                assert_eq!(body["contentHashAlgorithm"], "SHA256");
                uploads.push(body["name"].as_str().unwrap());
            }
            _ => panic!("unexpected mock request: {path}"),
        }
    }
    assert_eq!(ancestor_lookups, 2);
    assert_eq!(target_cursors, ["", "100", "100", "", "100", "", "100"]);
    assert_eq!(uploads, ["a.txt", "b.txt"]);
}

#[tokio::test]
async fn recursive_upload_finishes_the_paged_fixture_after_a_mid_upload_disconnect() {
    let server = Server::new();
    server.state.lock().unwrap().drop_list_cursor_once = Some("100".into());
    let source = tempfile::tempdir().unwrap();
    let child = source.path().join("child");
    std::fs::create_dir(&child).unwrap();
    std::fs::create_dir(source.path().join("empty")).unwrap();
    for index in 1..=121 {
        let name = format!("page-{index:03}.txt");
        std::fs::write(source.path().join(&name), format!("root {index}\n")).unwrap();
        std::fs::write(child.join(&name), format!("child {index}\n")).unwrap();
    }
    services::upload::upload_directory(&server.config(), source.path(), "/paged", true, false)
        .await
        .unwrap();

    let state = server.state.lock().unwrap();
    assert!(state.drop_list_cursor_once.is_none());
    let root_id = state.children["/"][0]["fileId"].as_str().unwrap();
    let root = &state.children[root_id];
    assert_eq!(root.len(), 123);
    let child = root.iter().find(|item| item["name"] == "child").unwrap();
    let child_id = child["fileId"].as_str().unwrap();
    assert_eq!(state.children[child_id].len(), 121);
    assert!(
        root.iter()
            .any(|item| item["name"] == "empty" && item["type"] == "folder")
    );
    let files: std::collections::HashSet<_> = state
        .requests
        .iter()
        .filter(|(path, body)| path == "/file/create" && body["type"] == "file")
        .map(|(_, body)| {
            let parent = body["parentFileId"].as_str().unwrap();
            let name = body["name"].as_str().unwrap();
            let local = if parent == child_id {
                source.path().join("child").join(name)
            } else {
                assert_eq!(parent, root_id);
                source.path().join(name)
            };
            let hash = cloud139::utils::crypto::calc_file_sha256(local.to_str().unwrap()).unwrap();
            assert_eq!(body["contentHash"], hash);
            (parent, name)
        })
        .collect();
    assert_eq!(files.len(), 242);
    assert_eq!(
        state
            .requests
            .iter()
            .filter(|(path, _)| path == "/file/create")
            .count(),
        245
    );
}

#[tokio::test]
async fn transfer_and_rename_preflight_types_conflicts_and_missing_sources() {
    let server = Server::new();
    server.set(
        "/",
        vec![folder("s", "src"), folder("d", "dest"), file("f", "file")],
    );
    server.set("d", vec![file("c", "src")]);
    assert!(
        services::directory::cp(&server.config(), &["/src".into()], "/dest", true)
            .await
            .is_err()
    );
    assert!(
        services::directory::mv(&server.config(), &["/src".into()], "/file", true)
            .await
            .is_err()
    );
    assert!(
        services::rename::rename(&server.config(), "/src", "file", true)
            .await
            .is_err()
    );
    assert!(
        services::rename::rename(&server.config(), "/src", "../unsafe", true)
            .await
            .is_err()
    );
    assert!(
        services::directory::cp(
            &server.config(),
            &["/src".into(), "/missing".into()],
            "/dest",
            true
        )
        .await
        .is_err()
    );
    assert!(server.mutations().is_empty());
    server.set("d", vec![]);
    services::directory::cp(&server.config(), &["/src".into()], "/dest", false)
        .await
        .unwrap();
    services::directory::mv(&server.config(), &["/src".into()], "/dest", false)
        .await
        .unwrap();
    services::rename::rename(&server.config(), "/src", "renamed", false)
        .await
        .unwrap();
    assert_eq!(server.mutations().len(), 3);
}

#[tokio::test]
async fn directory_delete_needs_recursive_and_permanent_never_trashes() {
    let server = Server::new();
    server.set("/", vec![folder("d", "dir")]);
    assert!(
        services::delete::delete(&server.config(), "/dir", false)
            .await
            .unwrap_err()
            .to_string()
            .contains("-r")
    );
    assert!(server.mutations().is_empty());
    services::delete::delete(&server.config(), "/dir", true)
        .await
        .unwrap();
    assert_eq!(server.mutations()[0].0, "/recyclebin/batchTrash");
}

#[cfg(unix)]
#[tokio::test]
async fn symlinks_in_upload_and_download_are_rejected() {
    use std::os::unix::fs::symlink;
    let server = Server::new();
    server.set("/", vec![folder("d", "dir")]);
    server.set("d", vec![file("f", "data.bin")]);
    let tmp = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("data.bin"), b"keep").unwrap();
    symlink(outside.path(), tmp.path().join("link")).unwrap();
    assert!(
        services::upload::upload_directory(&server.config(), tmp.path(), "/target", true, false)
            .await
            .is_err()
    );
    assert!(server.mutations().is_empty());
    assert!(
        services::download::download(
            &server.config(),
            "/dir",
            tmp.path().join("link/out").to_str().unwrap(),
            None,
            true
        )
        .await
        .is_err()
    );
    let out = tmp.path().join("out");
    std::fs::create_dir(&out).unwrap();
    symlink(outside.path().join("data.bin"), out.join("data.bin")).unwrap();
    assert!(
        services::download::download(&server.config(), "/dir", out.to_str().unwrap(), None, true)
            .await
            .is_err()
    );
    assert_eq!(
        std::fs::read(outside.path().join("data.bin")).unwrap(),
        b"keep"
    );
}

#[tokio::test]
async fn failed_or_missing_listing_data_is_never_treated_as_empty() {
    use httpmock::prelude::*;
    for response in [
        json!({"success":false,"message":"denied","data":{"items":[],"nextPageCursor":""}}),
        json!({"success":true}),
    ] {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST).path("/file/list");
            then.status(200).json_body(response);
        });
        let config = Config {
            account: "13800138000".into(),
            authorization: "fictional".into(),
            personal_cloud_host: Some(server.base_url()),
            ..Default::default()
        };
        assert!(
            cloud139::client::http::HttpClientWrapper::new()
                .list_personal_files(&config, "/")
                .await
                .is_err()
        );
        assert!(
            cloud139::client::api::files::get_file_id_by_path(&config, "/missing")
                .await
                .is_err()
        );
        mock.assert_calls(2);
    }
}

#[tokio::test]
async fn server_root_id_alias_is_rejected_before_delete_or_rename() {
    let server = Server::new();
    server.set("/", vec![folder("/", "alias")]);
    assert!(
        services::delete::delete(&server.config(), "/alias", true)
            .await
            .is_err()
    );
    assert!(
        services::rename::rename(&server.config(), "/alias", "renamed", false)
            .await
            .is_err()
    );
    assert!(server.mutations().is_empty());
}

#[test]
fn cli_delete_requires_yes_without_credentials() {
    let tmp = tempfile::tempdir().unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_cloud139"))
        .args(["rm", "-r", "/dir"])
        .current_dir(tmp.path())
        .env("HOME", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("--yes"));
}

#[test]
fn sync_preserves_excluded_local_subtree_and_skips_cleanup_on_conflict() {
    let server = Server::new();
    server.set("/", vec![file("replacement", "nested")]);
    let root = tempfile::tempdir().unwrap();
    let destination = root.path().join("destination");
    std::fs::create_dir_all(destination.join("nested")).unwrap();
    std::fs::write(destination.join("nested/unique.keep"), b"keep").unwrap();
    std::fs::write(destination.join("extra"), b"keep on failure").unwrap();

    let output = server
        .cli(root.path())
        .args(["sync", "cloud:/"])
        .arg(&destination)
        .args(["-r", "--delete", "--exclude", "*.keep"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        std::fs::read(destination.join("nested/unique.keep")).unwrap(),
        b"keep"
    );
    assert_eq!(
        std::fs::read(destination.join("extra")).unwrap(),
        b"keep on failure"
    );
    assert!(
        server
            .state
            .lock()
            .unwrap()
            .requests
            .iter()
            .all(|(path, _)| path == "/file/list")
    );
}

#[test]
fn sync_local_case_alias_is_never_downloaded_then_deleted() {
    let server = Server::new();
    server.set("/", vec![file("download", "LOWER.txt")]);
    let root = tempfile::tempdir().unwrap();
    let destination = root.path().join("destination");
    std::fs::create_dir(&destination).unwrap();
    let original = destination.join("lower.txt");
    let download = destination.join("LOWER.txt");
    std::fs::write(&original, b"keep").unwrap();
    let aliases = download.exists();
    let output = server
        .cli(root.path())
        .args(["sync", "cloud:/"])
        .arg(&destination)
        .args(["--delete", "-j", "1"])
        .output()
        .unwrap();

    if aliases {
        assert_eq!(output.status.code(), Some(2));
        assert_eq!(std::fs::read(&original).unwrap(), b"keep");
        assert!(
            server
                .state
                .lock()
                .unwrap()
                .requests
                .iter()
                .all(|(path, _)| path == "/file/list")
        );
    } else {
        assert!(output.status.success());
        assert!(!original.exists());
        assert_eq!(std::fs::read(&download).unwrap(), b"data");
    }
}

#[test]
fn ordinary_sync_refuses_to_delete_a_replacement_file() {
    let server = Server::new();
    server.set("/", vec![file("original", "extra")]);
    server.state.lock().unwrap().replace_after_scan = Some(("original", "different"));
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("empty");
    std::fs::create_dir(&source).unwrap();
    let output = server
        .cli(root.path())
        .arg("sync")
        .arg(&source)
        .args(["cloud:/", "--delete"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(server.mutations().is_empty());
    assert_eq!(
        server.state.lock().unwrap().children["/"][0]["fileId"],
        "different"
    );
}

#[test]
fn ordinary_sync_only_recycles_parent_after_its_children() {
    let server = Server::new();
    server.set("/", vec![folder("parent", "extra-dir")]);
    server.set("parent", vec![file("child", "extra-file")]);
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("empty");
    std::fs::create_dir(&source).unwrap();
    for fail in [true, false] {
        {
            let mut state = server.state.lock().unwrap();
            state.fail_trash_item = fail;
            state.sync_mutations = true;
            state.requests.clear();
        }
        let output = server
            .cli(root.path())
            .arg("sync")
            .arg(&source)
            .args(["cloud:/", "-r", "--delete", "-j", "4"])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(i32::from(fail)));
        let ids: Vec<_> = server
            .mutations()
            .into_iter()
            .map(|(_, body)| body["fileIds"].clone())
            .collect();
        assert_eq!(
            ids,
            if fail {
                vec![json!(["child"])]
            } else {
                vec![json!(["child"]), json!(["parent"])]
            }
        );
    }
}

#[test]
fn cloud_file_is_preserved_when_local_directory_has_the_same_name() {
    let server = Server::new();
    server.set("/", vec![file("existing", "nested")]);
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    std::fs::create_dir_all(source.join("nested")).unwrap();
    std::fs::write(source.join("nested/file"), b"content").unwrap();
    let output = server
        .cli(root.path())
        .arg("sync")
        .arg(&source)
        .args(["cloud:/", "-r"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(server.mutations().is_empty());
    assert_eq!(
        server.state.lock().unwrap().children["/"][0]["fileId"],
        "existing"
    );
}

#[test]
fn verified_sync_unchanged_files_only_list_during_the_scan() {
    let server = Server::new();
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let nested = source.join("nested");
    std::fs::create_dir_all(&nested).unwrap();
    server.set("/", vec![folder("backup-id", "backup")]);
    server.set("backup-id", vec![folder("nested-id", "nested")]);
    let bytes = b"unchanged";
    let mut files = Vec::new();
    for index in 0..302 {
        let name = format!("object-{index:03}.bin");
        std::fs::write(nested.join(&name), bytes).unwrap();
        files.push(verified_file(&format!("id-{index}"), &name, bytes, true));
    }
    server.set("nested-id", files);
    let report = sync_report(
        server
            .cli(root.path())
            .arg("sync")
            .arg(&source)
            .args(["cloud:/backup", "-r", "--verify", "--json", "--delete"])
            .output()
            .unwrap(),
        0,
    );
    assert_eq!(report["skipped"], 302);
    assert_eq!(report["changes"], json!([]));
    let state = server.state.lock().unwrap();
    let pages: Vec<_> = state
        .requests
        .iter()
        .map(|(path, body)| {
            assert_eq!(path, "/file/list");
            (
                body["parentFileId"].as_str().unwrap(),
                body["pageInfo"]["pageCursor"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        pages,
        [
            ("/", ""),
            ("backup-id", ""),
            ("nested-id", ""),
            ("nested-id", "100"),
            ("nested-id", "200"),
            ("nested-id", "300"),
        ]
    );
}

#[test]
fn verified_sync_uses_scan_matches_and_rechecks_them_before_deleting() {
    for delete in [false, true] {
        let server = Server::new();
        server.set(
            "/",
            vec![
                verified_file("original", "movie.bin", b"new content", true),
                file("extra", "extra.txt"),
            ],
        );
        server.state.lock().unwrap().replace_after_scan = Some(("original", "different"));
        let root = tempfile::tempdir().unwrap();
        let mut command = server.sync_command(root.path());
        command.args(["--verify", "--json"]);
        if delete {
            command.arg("--delete");
        }
        let report = sync_report(command.output().unwrap(), i32::from(delete));
        assert_eq!(report["skipped"], 1);
        if delete {
            assert_eq!(report["failures"][0]["path"], "movie.bin");
        } else {
            assert_eq!(report["failures"], json!([]));
        }
        assert!(server.mutations().is_empty());
    }
}

#[test]
fn verified_sync_refuses_to_replace_a_target_changed_after_scan() {
    let server = Server::new();
    server.set(
        "/",
        vec![verified_file("original", "movie.bin", b"old content", true)],
    );
    server.state.lock().unwrap().replace_after_scan = Some(("original", "different"));
    let root = tempfile::tempdir().unwrap();
    let output = server
        .sync_command(root.path())
        .args(["--verify", "--json"])
        .output()
        .unwrap();
    let report = sync_report(output, 1);
    assert_eq!(report["skipped"], 0);
    assert_eq!(report["failures"][0]["path"], "movie.bin");
    assert!(server.mutations().is_empty());
}

#[test]
fn sync_uploads_single_file_and_rejects_a_prune_of_the_entire_target() {
    let server = Server::new();
    server.state.lock().unwrap().sync_mutations = true;
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("sample.bin");
    std::fs::write(&source, b"data").unwrap();
    let mut command = server.cli(root.path());
    command.arg("sync").arg(&source).arg("cloud:/");
    let accepted = command.output().unwrap();
    assert!(
        accepted.status.success(),
        "{}",
        String::from_utf8_lossy(&accepted.stderr)
    );
    assert_eq!(
        server.state.lock().unwrap().children["/"][0]["name"],
        "sample.bin"
    );
    let refused = command.arg("--delete").output().unwrap();
    assert_eq!(refused.status.code(), Some(2));
}

#[test]
fn upload_refuses_a_success_response_without_a_receipt() {
    let server = Server::new();
    server.state.lock().unwrap().omit_upload_receipt = true;
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("sample.bin");
    std::fs::write(&source, b"data").unwrap();
    let result = server
        .cli(root.path())
        .arg("upload")
        .arg(&source)
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&result.stderr).contains("未返回上传结果"));
}

#[tokio::test]
async fn ordinary_multipart_upload_preserves_server_selected_name_and_never_trashes() {
    use httpmock::prelude::*;

    for force in [true, false] {
        let server = MockServer::start();
        let config = Config {
            personal_cloud_host: Some(server.base_url()),
            ..Config::default()
        };
        let list = server.mock(|when, then| {
            when.method(POST).path("/file/list");
            let items = if force {
                vec![file("old-id", "report.txt")]
            } else {
                // 检查时可用的名称可能在创建时已被占用。
                Vec::new()
            };
            then.json_body(json!({"success":true,"data":{"items":items}}));
        });
        let create = server.mock(|when, then| {
            when.method(POST).path("/file/create");
            then.json_body(json!({"success":true, "data":{
                "fileId":"new-id", "fileName":"report-copy.txt", "uploadId":"transfer",
                "partInfos":[{"partNumber":1, "uploadUrl":server.url("/part")}]
            }}));
        });
        let part = server.mock(|when, then| {
            when.method(PUT).path("/part").body("test content");
            then.status(200);
        });
        let finish = server.mock(|when, then| {
            when.method(POST).path("/file/complete");
            then.json_body(json!({"success":true}));
        });
        let rename = server.mock(|when, then| {
            when.path("/file/update");
            then.status(500);
        });
        let trash = server.mock(|when, then| {
            when.path("/recyclebin/batchTrash");
            then.status(500);
        });
        let local = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(local.path(), b"test content").unwrap();
        services::upload::upload(&config, local.path(), "/", "report.txt", 12, force, None)
            .await
            .unwrap();
        list.assert_calls(1);
        create.assert_calls(1);
        part.assert_calls(1);
        finish.assert_calls(1);
        rename.assert_calls(0);
        trash.assert_calls(0);
    }
}
