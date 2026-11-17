//! Stateful loopback-only regression tests. No real 139 credentials or endpoints.
use cloud139::{
    client::ClientError,
    config::Config,
    services::{self, directory_service as dirs},
};
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
    fail_batch: bool,
    fail_trash_item: bool,
    page_size: usize,
    cycle: bool,
    drop_list_cursor_once: Option<String>,
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
                                let items = state.children.get(parent).cloned().unwrap_or_default();
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
                                if state
                                    .fail_name
                                    .as_deref()
                                    .is_some_and(|name| body["name"] == name)
                                {
                                    json!({"success":false,"message":"injected failure"})
                                } else {
                                    let id = format!("id-{}", state.requests.len());
                                    let parent = body["parentFileId"].as_str().unwrap().to_owned();
                                    let item = json!({"fileId":id,"name":body["name"],"type":body["type"],"size":body["size"]});
                                    state.children.entry(parent).or_default().push(item);
                                    // Fast-upload mock acknowledges by hash without a data transfer.
                                    json!({"success":true})
                                }
                            }
                            "/file/getDownloadUrl" => {
                                json!({"success":true,"data":{"url":format!("{base}/payload"),"fileName":"safe.bin"}})
                            }
                            "/payload" => {
                                drop(state);
                                let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\ndata");
                                continue;
                            }
                            "/file/batchCopy" | "/file/batchMove" => {
                                json!({"success":!state.fail_batch,"message":"injected batch failure"})
                            }
                            "/file/update" => json!({"success":true}),
                            "/recyclebin/batchTrash" => {
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
                )
            })
            .cloned()
            .collect()
    }
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

#[test]
fn rejected_group_login_preserves_existing_profile() {
    use base64::Engine;

    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("profile.toml");
    let original = b"root_folder_id = \"root:/existing-group\"\n";
    std::fs::write(&config, original).unwrap();
    let token = base64::engine::general_purpose::STANDARD
        .encode("pc:13800138000:test|1|RCS|9999999999999|signature");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_cloud139"))
        .current_dir(temp.path())
        .env("HOME", temp.path())
        .env("XDG_CONFIG_HOME", temp.path())
        .arg("--config")
        .arg(&config)
        .args([
            "login",
            "--storage-type",
            "group",
            "--cloud-id",
            "different-group",
            "--token",
            &token,
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("root_folder_id"));
    assert_eq!(std::fs::read(&config).unwrap(), original);
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
        let result = services::download_service::download_recursive(
            &server.config(),
            "/folder",
            destination.to_str().unwrap(),
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
    let error = services::delete(&server.config(), "/report.txt", false)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("删除失败"));
    assert_eq!(server.mutations().len(), 1);
}

#[test]
fn normalizes_root_aliases_and_checks_names() {
    for path in ["", "/", "//", "/./", "/a/..", "a/../.", "/../../"] {
        assert_eq!(dirs::normalize(path).unwrap(), "/");
        assert!(matches!(
            dirs::non_root(path),
            Err(ClientError::CannotOperateOnRoot)
        ));
    }
    assert_eq!(dirs::normalize("//a/./b/../c/").unwrap(), "/a/c");
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
        assert!(dirs::validate_name(name).is_err(), "{name:?}");
    }
}

#[tokio::test]
async fn root_mutations_are_rejected_for_every_backend_without_network() {
    for storage in ["personal_new", "family", "group"] {
        let config = Config {
            storage_type: storage.into(),
            ..Default::default()
        };
        for root in ["/", "//./", "/a/..", ""] {
            assert!(
                services::delete_service::delete_recursive(&config, root, false, true)
                    .await
                    .unwrap_err()
                    .to_string()
                    .contains("根目录")
            );
            assert!(matches!(
                services::rename(&config, root, "safe", false).await,
                Err(ClientError::CannotOperateOnRoot)
            ));
            assert!(matches!(
                services::cp(&config, &[root.into()], "/dst", true).await,
                Err(ClientError::CannotOperateOnRoot)
            ));
            assert!(matches!(
                services::mv(&config, &[root.into()], "/dst", true).await,
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
            services::cp(&config, &["/a".into()], target, true)
                .await
                .unwrap_err()
                .to_string()
                .contains("自身或子目录")
        );
        assert!(
            services::mv(&config, &["/a".into()], target, true)
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
    services::mkdir_service::mkdir_parents(&config, "/a/b/empty")
        .await
        .unwrap();
    assert_eq!(server.mutations().len(), 3);
    services::mkdir_service::mkdir_parents(&config, "/a/b/empty/")
        .await
        .unwrap();
    assert_eq!(server.mutations().len(), 3);
    server.set("/", vec![file("f", "occupied")]);
    assert!(
        services::mkdir_service::mkdir_parents(&config, "/occupied/child")
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
    let result = services::list_service::list_recursive(&server.config(), "/")
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
        services::list_service::list_recursive(&server.config(), "/")
            .await
            .unwrap_err()
            .to_string()
            .contains("分页游标重复")
    );
    server.state.lock().unwrap().cycle = false;
    server.set("d", vec![folder("d", "loop")]);
    assert!(
        services::list_service::list_recursive(&server.config(), "/")
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
    let args = cloud139::commands::list::ListArgs {
        path: "/".into(),
        page: 1,
        page_size: 100,
        output: None,
        recursive: false,
    };
    assert!(
        services::list(&server.config(), &args)
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
        services::download_service::download_recursive(&server.config(), "/dir", local, false)
            .await
            .is_err()
    );
    assert!(!out.exists());
    services::download_service::download_recursive(&server.config(), "/dir", local, true)
        .await
        .unwrap();
    assert!(out.join("empty").is_dir());
    assert_eq!(std::fs::read(out.join("data.bin")).unwrap(), b"data");
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
            services::download_service::download_recursive(
                &server.config(),
                "/dir",
                out.to_str().unwrap(),
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
        services::download_service::download_recursive(
            &server.config(),
            "/dir",
            out.to_str().unwrap(),
            true
        )
        .await
        .is_err()
    );
    assert_eq!(std::fs::read(out.join("empty")).unwrap(), b"keep");
}

#[tokio::test]
async fn upload_maps_directory_or_contents_keeps_empty_and_never_skips_same_size() {
    let server = Server::new();
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("source");
    std::fs::create_dir_all(source.join("empty")).unwrap();
    std::fs::write(source.join("file.bin"), b"new!").unwrap();
    services::upload_service::upload_directory(
        &server.config(),
        &source,
        "/missing/target",
        false,
        false,
    )
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
    services::upload_service::upload_directory(
        &server.config(),
        &source,
        "/missing/target",
        false,
        true,
    )
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
    services::upload_service::upload_directory(&second.config(), &source, "/target", true, false)
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
    server.state.lock().unwrap().fail_name = Some("z.bin".into());
    let error = services::upload_service::upload_directory(
        &server.config(),
        tmp.path(),
        "/target",
        true,
        false,
    )
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
    server.set("/", vec![folder("s", "src"), folder("d", "dest")]);
    server.state.lock().unwrap().fail_batch = true;
    assert!(
        services::cp(&server.config(), &["/src".into()], "/dest", false)
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
    let error = services::upload_service::upload_directory(
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
    assert!(message.contains("云端已有同名条目"), "{message}");

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
    services::upload_service::upload_directory(
        &server.config(),
        source.path(),
        "/paged",
        true,
        false,
    )
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
        services::cp(&server.config(), &["/src".into()], "/dest", true)
            .await
            .is_err()
    );
    assert!(
        services::mv(&server.config(), &["/src".into()], "/file", true)
            .await
            .is_err()
    );
    assert!(
        services::rename(&server.config(), "/src", "file", true)
            .await
            .is_err()
    );
    assert!(
        services::rename(&server.config(), "/src", "../unsafe", true)
            .await
            .is_err()
    );
    assert!(
        services::cp(
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
    services::cp(&server.config(), &["/src".into()], "/dest", false)
        .await
        .unwrap();
    services::mv(&server.config(), &["/src".into()], "/dest", false)
        .await
        .unwrap();
    services::rename(&server.config(), "/src", "renamed", false)
        .await
        .unwrap();
    assert_eq!(server.mutations().len(), 3);
}

#[tokio::test]
async fn directory_delete_needs_recursive_and_permanent_never_trashes() {
    let server = Server::new();
    server.set("/", vec![folder("d", "dir")]);
    assert!(
        services::delete(&server.config(), "/dir", false)
            .await
            .unwrap_err()
            .to_string()
            .contains("-r")
    );
    assert!(
        services::delete_service::delete_recursive(&server.config(), "/dir", true, true)
            .await
            .unwrap_err()
            .to_string()
            .contains("永久删除")
    );
    assert!(server.mutations().is_empty());
    services::delete_service::delete_recursive(&server.config(), "/dir", false, true)
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
        services::upload_service::upload_directory(
            &server.config(),
            tmp.path(),
            "/target",
            true,
            false
        )
        .await
        .is_err()
    );
    assert!(server.mutations().is_empty());
    assert!(
        services::download_service::download_recursive(
            &server.config(),
            "/dir",
            tmp.path().join("link/out").to_str().unwrap(),
            true
        )
        .await
        .is_err()
    );
    let out = tmp.path().join("out");
    std::fs::create_dir(&out).unwrap();
    symlink(outside.path().join("data.bin"), out.join("data.bin")).unwrap();
    assert!(
        services::download_service::download_recursive(
            &server.config(),
            "/dir",
            out.to_str().unwrap(),
            true
        )
        .await
        .is_err()
    );
    assert_eq!(
        std::fs::read(outside.path().join("data.bin")).unwrap(),
        b"keep"
    );
}

#[test]
fn recursive_cli_flags_parse() {
    use clap::Parser;
    assert!(cloud139::commands::list::ListArgs::parse_from(["ls", "-R"]).recursive);
    assert!(
        cloud139::commands::download::DownloadArgs::parse_from(["download", "-r", "/dir"])
            .recursive
    );
    assert!(cloud139::commands::mkdir::MkdirArgs::parse_from(["mkdir", "-p", "/a/b"]).parents);
    let args = cloud139::commands::delete::DeleteArgs::parse_from(["rm", "-r", "--yes", "/dir"]);
    assert!(args.recursive && args.yes);
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
            cloud139::client::api::list_personal_files(&config, "/")
                .await
                .is_err()
        );
        assert!(
            cloud139::client::api::get_file_id_by_path(&config, "/missing")
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
        services::delete_service::delete_recursive(&server.config(), "/alias", false, true)
            .await
            .is_err()
    );
    assert!(
        services::rename(&server.config(), "/alias", "renamed", false)
            .await
            .is_err()
    );
    assert!(server.mutations().is_empty());
}

#[test]
fn cli_delete_requires_yes_and_rejects_permanent_without_credentials() {
    let tmp = tempfile::tempdir().unwrap();
    for (args, expected) in [
        (vec!["rm", "-r", "/dir"], "--yes"),
        (vec!["rm", "--yes", "--permanent", "/file"], "永久删除"),
    ] {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_cloud139"))
            .args(args)
            .current_dir(tmp.path())
            .env("HOME", tmp.path())
            .env("XDG_CONFIG_HOME", tmp.path())
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(text.contains(expected), "{text}");
        assert!(!text.contains("已永久删除"));
    }
}
