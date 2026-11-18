use cloud139::{
    client::{CLIENT_INFO, DEVICE_INFO, PC_CLIENT_INFO, api, auth, headers},
    config::Config,
    domain::StorageType,
};
use httpmock::prelude::*;
use serde_json::json;

#[test]
fn pc_identity_is_personal_only_and_keeps_auth_and_legacy_protocol_fields() {
    for (svc, expected) in [
        ("1", PC_CLIENT_INFO),
        ("2", CLIENT_INFO),
        ("3", CLIENT_INFO),
    ] {
        let h = headers::build_signed_headers(
            "existing-token",
            "timestamp",
            "random",
            "signature",
            svc,
        )
        .unwrap();
        assert_eq!(h["x-yun-client-info"], expected);
        assert_eq!(h["Authorization"], "Basic existing-token");
        assert_eq!(h["mcloud-sign"], "timestamp,random,signature");
        assert_eq!(h["x-DeviceInfo"], DEVICE_INFO);
        assert_eq!(h["Caller"], "web");
    }
    let route =
        headers::build_route_headers("existing-token", "timestamp", "random", "signature").unwrap();
    assert!(!route.contains_key("x-yun-client-info"));
    assert_eq!(route["x-DeviceInfo"], DEVICE_INFO);
    let group = headers::build_group_signed_headers(
        "existing-token",
        "timestamp",
        "random",
        "signature",
        "3",
    )
    .unwrap();
    assert!(!group.contains_key("x-yun-client-info"));
}

#[tokio::test]
async fn imported_token_and_refreshed_token_both_work_with_pc_file_requests() {
    use base64::Engine;
    let server = MockServer::start();
    let token = base64::engine::general_purpose::STANDARD
        .encode("pc:13800138000:old|1|RCS|9999999999999|signature");
    let mut config = auth::login(&token, "personal_new", None).await.unwrap();
    config.personal_cloud_host = Some(server.base_url());
    let old_list = server.mock(|when, then| {
        when.method(POST)
            .path("/file/list")
            .header("Authorization", format!("Basic {token}"))
            .header("x-yun-client-info", PC_CLIENT_INFO);
        then.json_body(json!({"success":true,"data":{"items":[],"nextPageCursor":""}}));
    });
    api::HttpClientWrapper::new()
        .list_personal_files(&config, "/")
        .await
        .unwrap();
    old_list.assert_calls(1);
    let refresh = server.mock(|when, then| {
        when.method(POST).path("/refresh")
            .body("<root><token>old|1|RCS|9999999999999|signature</token><account>13800138000</account><clienttype>656</clienttype></root>");
        then.body("<root><return>0</return><token>new-token</token><expiretime>2592000</expiretime></root>");
    });
    let refreshed = auth::refresh_token(
        &config,
        auth::RefreshOptions {
            endpoint: Some(&server.url("/refresh")),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    refresh.assert_calls(1);
    let auth_value = format!(
        "Basic {}",
        base64::engine::general_purpose::STANDARD.encode("pc:13800138000:new-token")
    );
    for endpoint in [
        "/file/list",
        "/file/update",
        "/file/getUploadUrl",
        "/file/complete",
        "/file/getDownloadUrl",
        "/recyclebin/batchTrash",
    ] {
        let request = server.mock(|when, then| {
            when.method(POST)
                .path(endpoint)
                .header("Authorization", &auth_value)
                .header("x-yun-client-info", PC_CLIENT_INFO);
            then.json_body(json!({"success":true}));
        });
        let response: serde_json::Value = api::HttpClientWrapper::new()
            .personal_api_request(
                &refreshed,
                &server.url(endpoint),
                json!({}),
                StorageType::PersonalNew,
            )
            .await
            .unwrap();
        assert_eq!(response["success"], true);
        request.assert_calls(1);
    }
}

#[cfg(unix)]
#[test]
fn local_metadata_no_longer_rejects_files_at_web_account_boundaries() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("sparse.bin");
    let file = std::fs::File::create(&path).unwrap();
    for size in [5_u64, 8, 50, 500] {
        let bytes = size * 1024 * 1024 * 1024 + 1;
        file.set_len(bytes).unwrap();
        assert_eq!(
            cloud139::services::upload_service::checked_upload_size(&path).unwrap(),
            bytes as i64
        );
    }
}

#[tokio::test]
async fn large_file_upload_urls_include_part_101_and_tail_size() {
    let server = MockServer::start();
    let config = Config {
        account: "13800138000".into(),
        ..Default::default()
    };
    let part_size = 512_i64 * 1024 * 1024;
    let size = 100 * part_size + 1;
    let mut mocks = Vec::new();
    for (start, end) in [(1, 100), (101, 101)] {
        let infos: Vec<_> = (start..=end)
            .map(|n| {
                json!({
                    "partNumber": n, "partSize": if n == 101 { 1 } else { part_size },
                    "parallelHashCtx": { "partOffset": (n - 1) as i64 * part_size }
                })
            })
            .collect();
        let response: Vec<_> = (start..=end)
            .map(|n| {
                json!({
                    "partNumber": n, "uploadUrl": format!("https://example.invalid/part-{n}")
                })
            })
            .collect();
        mocks.push(server.mock(|when, then| {
            when.method(POST)
                .path("/file/getUploadUrl")
                .header("x-yun-client-info", PC_CLIENT_INFO)
                .json_body(
                    json!({"fileId":"file","uploadId":"upload","partInfos":infos,
                    "commonAccountInfo":{"account":"13800138000","accountType":1}}),
                );
            then.json_body(json!({"success":true,"data":{"partInfos":response}}));
        }));
    }
    let urls = cloud139::services::upload::personal_parts::get_upload_urls(
        &config,
        &server.base_url(),
        "file",
        "upload",
        101,
        part_size,
        size,
    )
    .await
    .unwrap();
    assert_eq!(urls.len(), 101);
    assert!(urls.contains_key(&100));
    assert_eq!(urls[&101], "https://example.invalid/part-101");
    for m in mocks {
        m.assert_calls(1);
    }
}

#[tokio::test]
async fn sync_transfer_and_completion_failures_never_delete_existing_file() {
    use cloud139::domain::{ChangeKind, SyncAction};
    use cloud139::services::sync_executor::{SyncExecuteOptions, execute_sync_actions};
    for transfer_fails in [true, false] {
        let server = MockServer::start();
        let config = Config {
            personal_cloud_host: Some(server.base_url()),
            ..Default::default()
        };
        let local = tempfile::tempdir().unwrap();
        let source = local.path().join("old.bin");
        std::fs::write(&source, b"new").unwrap();
        let list = server.mock(|when, then| {
            when.method(POST).path("/file/list");
            then.json_body(json!({"success":true,"data":{"items":[{"fileId":"old","name":"old.bin","type":"file","size":4}]}}));
        });
        let create = server.mock(|when, then| {
            when.method(POST).path("/file/create");
            then.json_body(json!({"success":true,"data":{"fileId":"new","uploadId":"upload","partInfos":[{"partNumber":1,"uploadUrl":server.url("/part")}]}}));
        });
        let urls = server.mock(|when, then| {
            when.method(POST).path("/file/getUploadUrl");
            then.json_body(json!({"success":true,"data":{"partInfos":[{"partNumber":1,"uploadUrl":server.url("/part")}]}}));
        });
        let part = server.mock(|when, then| {
            when.method(PUT).path("/part").body("new");
            then.status(if transfer_fails { 500 } else { 200 });
        });
        let complete = server.mock(|when, then| {
            when.method(POST).path("/file/complete");
            then.json_body(json!({"success":false,"code":"04010319","message":"权益不足"}));
        });
        let rename = server.mock(|when, then| {
            when.path("/file/update");
            then.status(500);
        });
        let trash = server.mock(|when, then| {
            when.path("/recyclebin/batchTrash");
            then.status(500);
        });
        let summary = execute_sync_actions(
            &config,
            vec![SyncAction::Upload {
                rel_path: "old.bin".into(),
                local_abs: source,
                remote_abs: "/old.bin".into(),
                size: 3,
                change: ChangeKind::SizeOrTime,
                replace_directory_id: None,
            }],
            SyncExecuteOptions {
                dry_run: false,
                jobs: 1,
                progress: false,
                print_actions: false,
            },
        )
        .await
        .unwrap();
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.transferred, 0);
        list.assert_calls(1);
        create.assert_calls(1);
        urls.assert_calls(0);
        part.assert_calls(1);
        complete.assert_calls(usize::from(!transfer_fails));
        rename.assert_calls(0);
        trash.assert_calls(0);
    }
}
