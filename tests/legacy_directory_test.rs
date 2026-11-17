//! Family/group protocol fixtures exercise the same backend with an injected mock transport.
use cloud139::{
    client::ClientError,
    config::Config,
    services::{
        directory_service as dirs,
        legacy_directory::{LegacyBackend, LegacyTransport, list_body, parse_page, root},
    },
};
use serde_json::{Value, json};
use std::{collections::HashMap, sync::Mutex};

#[derive(Default)]
struct Fixture {
    pages: HashMap<(String, i64), Value>,
    requests: Vec<(String, Value)>,
    mutations: usize,
    fail: bool,
}
struct MockTransport(Mutex<Fixture>);
#[async_trait::async_trait]
impl LegacyTransport for MockTransport {
    async fn request(&self, url: &str, body: Value) -> Result<Value, ClientError> {
        let mut state = self.0.lock().unwrap();
        state.requests.push((url.into(), body.clone()));
        if url.ends_with("queryContentList") || url.ends_with("queryGroupContentList") {
            let id = body["catalogID"].as_str().unwrap().to_owned();
            let page = body["pageInfo"]["pageNum"]
                .as_i64()
                .unwrap_or_else(|| (body["startNumber"].as_i64().unwrap() - 1) / 100 + 1);
            return state
                .pages
                .get(&(id, page))
                .cloned()
                .ok_or_else(|| ClientError::Other("unexpected mock listing".into()));
        }
        state.mutations += 1;
        if !state.fail && (url.ends_with("createCloudDoc") || url.ends_with("createGroupCatalog")) {
            let family = url.ends_with("createCloudDoc");
            let wire = body["path"].as_str().unwrap().to_owned();
            let parent = if family {
                if wire == "root:/cloud" {
                    String::new()
                } else {
                    wire.rsplit('/').next().unwrap().to_owned()
                }
            } else {
                body["parentFileId"].as_str().unwrap().to_owned()
            };
            let id = format!("new-{}", state.mutations);
            let name = body[if family { "docLibName" } else { "catalogName" }]
                .as_str()
                .unwrap();
            let full = format!("{wire}/{id}");
            let page = state.pages.get_mut(&(parent, 1)).unwrap();
            if family {
                page["data"]["cloudCatalogList"]
                    .as_array_mut()
                    .unwrap()
                    .push(fcat(&id, name));
                page["data"]["totalCount"] =
                    json!(page["data"]["cloudCatalogList"].as_array().unwrap().len());
                state
                    .pages
                    .insert((id.clone(), 1), family_page(vec![], vec![], &full, 0));
                state
                    .pages
                    .insert((id, 2), family_page(vec![], vec![], &full, 0));
            } else {
                page["data"]["getGroupContentResult"]["catalogList"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"catalogID":id,"catalogName":name,"path":full}));
                page["data"]["getGroupContentResult"]["nodeCount"] = json!(
                    page["data"]["getGroupContentResult"]["catalogList"]
                        .as_array()
                        .unwrap()
                        .len()
                        + page["data"]["getGroupContentResult"]["contentList"]
                            .as_array()
                            .unwrap()
                            .len()
                );
                state
                    .pages
                    .insert((id, 1), group_page(vec![], vec![], &full, 0));
            }
        }
        Ok(json!({"result":{"resultCode":if state.fail {"1"} else {"0"}}}))
    }
}
fn family() -> Config {
    Config {
        storage_type: "family".into(),
        cloud_id: Some("cloud-id".into()),
        account: "13800138000".into(),
        authorization: "fictional".into(),
        ..Default::default()
    }
}
fn group() -> Config {
    Config {
        storage_type: "group".into(),
        cloud_id: Some("g".into()),
        root_folder_id: Some("root:/g".into()),
        ..family()
    }
}
fn fcat(id: &str, name: &str) -> Value {
    json!({"catalogID":id,"catalogName":name})
}
fn ffile(id: &str, name: &str) -> Value {
    json!({"contentID":id,"contentName":name,"contentSize":4})
}
fn family_page(catalogs: Vec<Value>, files: Vec<Value>, path: &str, total: i64) -> Value {
    json!({"data":{"result":{"resultCode":"0"},"path":path,"cloudCatalogList":catalogs,"cloudContentList":files,"totalCount":total}})
}
fn group_page(catalogs: Vec<Value>, files: Vec<Value>, path: &str, total: i64) -> Value {
    json!({"data":{"result":{"resultCode":"0"},"getGroupContentResult":{"parentCatalogID":path,"catalogList":catalogs,"contentList":files,"nodeCount":total}}})
}
fn group_fixture() -> MockTransport {
    let mut state = Fixture::default();
    state.pages.insert(
        ("g".into(), 1),
        group_page(
            vec![
                json!({"catalogID":"s-id","catalogName":"source","path":"root:/g/s-id"}),
                json!({"catalogID":"d-id","catalogName":"dest","path":"root:/g/d-id"}),
            ],
            vec![ffile("f-id", "file.bin")],
            "root:/g",
            3,
        ),
    );
    state.pages.insert(
        ("d-id".into(), 1),
        group_page(vec![], vec![], "root:/g/d-id", 0),
    );
    MockTransport(Mutex::new(state))
}

#[tokio::test]
async fn family_resolves_names_on_later_pages_to_real_ids_and_paths() {
    let config = family();
    let mut state = Fixture::default();
    state.pages.insert(
        ("".into(), 1),
        family_page(vec![fcat("skip-id", "skip")], vec![], "root:/cloud", 1),
    );
    state.pages.insert(
        ("".into(), 2),
        family_page(vec![fcat("parent-id", "parent")], vec![], "root:/cloud", 1),
    );
    state.pages.insert(
        ("".into(), 3),
        family_page(vec![], vec![], "root:/cloud", 0),
    );
    state.pages.insert(
        ("parent-id".into(), 1),
        family_page(
            vec![],
            vec![ffile("file-id", "leaf.bin")],
            "root:/cloud/parent-id",
            1,
        ),
    );
    state.pages.insert(
        ("parent-id".into(), 2),
        family_page(vec![], vec![], "root:/cloud/parent-id", 0),
    );
    let transport = MockTransport(Mutex::new(state));
    let api = LegacyBackend {
        config: &config,
        transport: &transport,
    };
    let entry = api.resolve("/parent/leaf.bin").await.unwrap();
    assert_eq!(entry.id, "file-id");
    assert_eq!(entry.wire_path, "root:/cloud/parent-id");
    let state = transport.0.lock().unwrap();
    assert_eq!(state.requests.len(), 5);
    assert!(
        state
            .requests
            .iter()
            .all(|(_, b)| b["catalogID"] != "parent" && b["catalogID"] != "/parent")
    );
    assert!(
        state
            .requests
            .iter()
            .all(|(_, b)| b["commonAccountInfo"]["account"] == "13800138000"
                && b["cloudID"] == "cloud-id")
    );
}

#[tokio::test]
async fn family_and_group_repeated_pages_fail_instead_of_looping() {
    for config in [family(), group()] {
        let mut state = Fixture::default();
        let is_family = config.storage_type == "family";
        let page = if is_family {
            family_page(vec![fcat("id", "name")], vec![], "root:/cloud", 200)
        } else {
            group_page(
                vec![json!({"catalogID":"id","catalogName":"name","path":"root:/g/id"})],
                vec![],
                "root:/g",
                200,
            )
        };
        let id = if is_family { "" } else { "g" };
        state.pages.insert((id.into(), 1), page.clone());
        state.pages.insert((id.into(), 2), page);
        let transport = MockTransport(Mutex::new(state));
        let api = LegacyBackend {
            config: &config,
            transport: &transport,
        };
        assert!(
            api.list_all(&root(&config).unwrap())
                .await
                .unwrap_err()
                .to_string()
                .contains("分页重复")
        );
        assert_eq!(transport.0.lock().unwrap().requests.len(), 2);
    }
}

#[tokio::test]
async fn group_move_uses_directory_ids_and_server_destination_path() {
    let config = group();
    let transport = group_fixture();
    let api = LegacyBackend {
        config: &config,
        transport: &transport,
    };
    api.move_group(&["/source".into()], "/dest").await.unwrap();
    let state = transport.0.lock().unwrap();
    let (_, body) = state.requests.last().unwrap();
    assert_eq!(body["catalogList"], json!(["s-id"]));
    assert_eq!(body["contentList"], json!([]));
    assert_eq!(body["destPath"], "root:/g/d-id");
    assert_eq!(body["taskType"], 3);
    assert_eq!(state.mutations, 1);
}

#[tokio::test]
async fn group_rename_and_trash_directories_use_verified_shapes() {
    let config = group();
    let transport = group_fixture();
    let api = LegacyBackend {
        config: &config,
        transport: &transport,
    };
    api.rename("/source", "renamed").await.unwrap();
    assert!(api.trash("/source", false).await.is_err());
    api.trash("/source", true).await.unwrap();
    let state = transport.0.lock().unwrap();
    let (_, rename) = state
        .requests
        .iter()
        .find(|(u, _)| u.ends_with("modifyGroupCatalog"))
        .unwrap();
    assert_eq!(rename["modifyCatalogID"], "s-id");
    assert_eq!(rename["modifyCatalogName"], "renamed");
    let (_, trash) = state.requests.last().unwrap();
    assert_eq!(trash["catalogList"], json!(["root:/g/s-id"]));
    assert_eq!(trash["taskType"], 2);
    assert_eq!(state.mutations, 2);
}

#[tokio::test]
async fn family_directory_rename_is_explicitly_unsupported_and_trash_uses_id() {
    let config = family();
    let mut state = Fixture::default();
    state.pages.insert(
        ("".into(), 1),
        family_page(vec![fcat("d-id", "dir")], vec![], "root:/cloud", 1),
    );
    state.pages.insert(
        ("".into(), 2),
        family_page(vec![], vec![], "root:/cloud", 0),
    );
    let transport = MockTransport(Mutex::new(state));
    let api = LegacyBackend {
        config: &config,
        transport: &transport,
    };
    assert!(matches!(
        api.rename("/dir", "renamed").await,
        Err(ClientError::UnsupportedFamilyRenameFolder)
    ));
    assert_eq!(transport.0.lock().unwrap().mutations, 0);
    api.trash("/dir", true).await.unwrap();
    let state = transport.0.lock().unwrap();
    let (_, trash) = state.requests.last().unwrap();
    assert_eq!(trash["catalogList"], json!(["d-id"]));
    assert_eq!(trash["path"], "root:/cloud");
    assert_eq!(trash["taskType"], 2);
}

#[tokio::test]
async fn legacy_errors_and_invalid_names_never_become_success() {
    let config = group();
    let transport = group_fixture();
    transport.0.lock().unwrap().fail = true;
    let api = LegacyBackend {
        config: &config,
        transport: &transport,
    };
    assert!(api.rename("/source", "renamed").await.is_err());
    assert!(api.rename("/source", "../bad").await.is_err());
    assert!(
        api.move_group(&["/source".into()], "/source/child")
            .await
            .is_err()
    );
    assert!(api.trash("/source/..", true).await.is_err());
    assert_eq!(transport.0.lock().unwrap().mutations, 1);
}

#[test]
fn parsers_reject_unsafe_names_missing_status_and_group_root_guessing() {
    for name in ["../escape", "/absolute", "a\\b", "NUL"] {
        let value = family_page(vec![fcat("id", name)], vec![], "root:/cloud", 1);
        assert!(parse_page(&family(), &value).is_err());
    }
    assert!(parse_page(&family(), &json!({"data":{}})).is_err());
    let mut config = group();
    config.root_folder_id = None;
    assert!(root(&config).is_err());
    let config = group();
    let directory = root(&config).unwrap();
    let body = list_body(&config, &directory, 2, 100);
    assert_eq!(body["catalogID"], "g");
    assert_eq!(body["startNumber"], 101);
    assert_eq!(body["endNumber"], 200);
    assert_eq!(body["path"], "root:/g");
    assert!(!dirs::mutation_succeeded(
        &json!({"success":true,"data":{"results":[{"success":true},{"success":false}]}})
    ));
}

#[tokio::test]
async fn legacy_mkdir_parents_is_idempotent_and_uses_resolved_parent_paths() {
    for config in [family(), group()] {
        let family = config.storage_type == "family";
        let transport = if family {
            let mut state = Fixture::default();
            state.pages.insert(
                ("".into(), 1),
                family_page(vec![], vec![], "root:/cloud", 0),
            );
            state.pages.insert(
                ("".into(), 2),
                family_page(vec![], vec![], "root:/cloud", 0),
            );
            MockTransport(Mutex::new(state))
        } else {
            group_fixture()
        };
        let api = LegacyBackend {
            config: &config,
            transport: &transport,
        };
        api.ensure_dir("/parent/empty", true).await.unwrap();
        api.ensure_dir("/parent/empty", true).await.unwrap();
        let state = transport.0.lock().unwrap();
        assert_eq!(state.mutations, 2);
        let changes: Vec<_> = state
            .requests
            .iter()
            .filter(|(u, _)| u.ends_with("createCloudDoc") || u.ends_with("createGroupCatalog"))
            .collect();
        assert_eq!(
            changes[0].1["path"],
            if family { "root:/cloud" } else { "root:/g" }
        );
        assert_eq!(
            changes[1].1["path"],
            if family {
                "root:/cloud/new-1"
            } else {
                "root:/g/new-1"
            }
        );
        if !family {
            assert_eq!(changes[1].1["parentFileId"], "new-1");
        }
    }
}

#[tokio::test]
async fn group_incomplete_pagination_is_an_error() {
    let config = group();
    let mut state = Fixture::default();
    state
        .pages
        .insert(("g".into(), 1), group_page(vec![], vec![], "root:/g", 20));
    let transport = MockTransport(Mutex::new(state));
    let api = LegacyBackend {
        config: &config,
        transport: &transport,
    };
    assert!(
        api.list_all(&root(&config).unwrap())
            .await
            .unwrap_err()
            .to_string()
            .contains("分页提前结束")
    );
}
