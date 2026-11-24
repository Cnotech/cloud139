use cloud139::config::Config;
use cloud139::models::sync_item::{ChangeKind, SyncAction, SyncSummary, SyncTarget};
use cloud139::services::sync::{
    executor::{SyncExecuteOptions, execute_sync_actions},
    plan::{SyncOutcome, SyncRequest},
    report::FileOperation,
    scan::SyncScanOptions,
};
use tempfile::tempdir;

#[tokio::test]
async fn sync_preview_reports_download_and_pruning_without_touching_files() {
    use httpmock::prelude::*;
    use serde_json::json;

    let server = MockServer::start();
    let listing = server.mock(|when, then| {
        when.method(POST)
            .path("/file/list")
            .json_body_includes(r#"{"parentFileId":"/"}"#);
        then.json_body(json!({"success": true, "data": {"items": [
            {"fileId":"new","name":"new.txt","type":"file","size":3}
        ]}}));
    });
    let download = server.mock(|when, then| {
        when.method(POST).path("/file/getDownloadUrl");
        then.status(500);
    });
    let config = Config {
        personal_cloud_host: Some(server.base_url()),
        ..Config::default()
    };
    let local = tempdir().unwrap();
    let extra = local.path().join("extra.txt");
    std::fs::write(&extra, "keep").unwrap();
    let request = SyncRequest::new(
        "cloud:/",
        local.path().to_str().unwrap(),
        SyncScanOptions {
            recursive: true,
            exclude: vec![],
        },
        SyncExecuteOptions {
            dry_run: true,
            jobs: 1,
            print_actions: false,
        },
        true,
        false,
    )
    .unwrap();
    let plan = request.plan(config, true).await.unwrap();
    let SyncOutcome::Report(report) = plan.execute().await.unwrap() else {
        panic!("跟踪路径的预览应返回报告");
    };

    assert!(report.dry_run);
    assert_eq!(
        (report.objects, report.downloaded, report.deleted),
        (1, 1, 1)
    );
    assert!(report.changes.is_empty());
    assert_eq!(report.plan.len(), 2);
    assert!(
        report
            .plan
            .iter()
            .any(|item| item.path == "new.txt" && item.action == FileOperation::Download)
    );
    assert!(
        report
            .plan
            .iter()
            .any(|item| item.path == "extra.txt" && item.action == FileOperation::Delete)
    );
    assert_eq!(std::fs::read(&extra).unwrap(), b"keep");
    assert!(!local.path().join("new.txt").exists());
    listing.assert_calls(1);
    download.assert_calls(0);
}

#[tokio::test]
async fn explicit_download_action_resolves_path_and_preserves_cloud_mtime() {
    use httpmock::prelude::*;
    use serde_json::json;

    let server = MockServer::start();
    let listing = server.mock(|when, then| {
        when.method(POST)
            .path("/file/list")
            .json_body_includes(r#"{"parentFileId":"/"}"#);
        then.json_body(json!({"success": true, "data": {"items": [
            {"fileId":"note","name":"note.txt","type":"file","size":3}
        ]}}));
    });
    let link = server.mock(|when, then| {
        when.method(POST)
            .path("/file/getDownloadUrl")
            .json_body(json!({"fileId":"note"}));
        then.json_body(json!({"success": true, "data": {"url": server.url("/blob")}}));
    });
    let blob = server.mock(|when, then| {
        when.method(GET).path("/blob");
        then.body("abc");
    });
    let config = Config {
        personal_cloud_host: Some(server.base_url()),
        ..Config::default()
    };
    let local = tempdir().unwrap();
    let local_abs = local.path().join("note.txt");
    let mtime = 1_704_067_200;
    let action = SyncAction::Download {
        rel_path: "note.txt".into(),
        remote_abs: "/note.txt".into(),
        local_abs: local_abs.clone(),
        size: 3,
        cloud_mtime: Some(mtime),
        change: ChangeKind::New,
    };
    let summary = execute_sync_actions(
        &config,
        vec![action],
        SyncExecuteOptions {
            dry_run: false,
            jobs: 1,
            print_actions: false,
        },
    )
    .await
    .unwrap();

    assert_eq!(
        (summary.transferred, summary.bytes, summary.failed),
        (1, 3, 0)
    );
    assert_eq!(std::fs::read(&local_abs).unwrap(), b"abc");
    let metadata = std::fs::metadata(&local_abs).unwrap();
    assert_eq!(
        filetime::FileTime::from_last_modification_time(&metadata).unix_seconds(),
        mtime
    );
    listing.assert_calls(1);
    link.assert_calls(1);
    blob.assert_calls(1);
}

#[tokio::test]
async fn test_execute_sync_actions_dry_run_counts_transfer_delete_and_skip() {
    let actions = vec![
        SyncAction::CreateDir {
            rel_path: "empty".to_string(),
            target: SyncTarget::Cloud,
            target_abs: "/remote/empty".to_string(),
        },
        SyncAction::Upload {
            rel_path: "new.txt".to_string(),
            local_abs: std::path::PathBuf::from("local/new.txt"),
            remote_abs: "/remote/new.txt".to_string(),
            size: 5,
            change: ChangeKind::New,
            replace_directory_id: None,
        },
        SyncAction::Delete {
            rel_path: "old.txt".to_string(),
            target: SyncTarget::Cloud,
            target_abs: "/remote/old.txt".to_string(),
        },
        SyncAction::Skip {
            rel_path: "same.txt".to_string(),
        },
    ];

    let summary = execute_sync_actions(
        &Config::default(),
        actions,
        SyncExecuteOptions {
            dry_run: true,
            jobs: 2,

            print_actions: false,
        },
    )
    .await
    .unwrap();

    assert_eq!(
        summary,
        SyncSummary {
            transferred: 1,
            skipped: 1,
            deleted: 1,
            created_dirs: 1,
            failed: 0,
            bytes: 5,
        }
    );
}

#[tokio::test]
async fn missing_local_upload_preserves_cloud_objects_and_other_actions_continue() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    let requests = server.mock(|when, then| {
        when.any_request();
        then.status(500);
    });
    let config = Config {
        personal_cloud_host: Some(server.base_url()),
        ..Config::default()
    };
    let tmp = tempdir().unwrap();
    let actions = vec![
        SyncAction::Upload {
            rel_path: "missing.bin".into(),
            local_abs: tmp.path().join("missing.bin"),
            remote_abs: "/existing/missing.bin".into(),
            size: 1,
            change: ChangeKind::SizeOrTime,
            replace_directory_id: None,
        },
        SyncAction::CreateDir {
            rel_path: "empty".into(),
            target: SyncTarget::Local,
            target_abs: tmp.path().join("empty").to_str().unwrap().into(),
        },
    ];
    let summary = execute_sync_actions(
        &config,
        actions,
        SyncExecuteOptions {
            dry_run: false,
            jobs: 2,

            print_actions: false,
        },
    )
    .await
    .unwrap();
    assert_eq!(summary.failed, 1);
    assert_eq!(summary.transferred, 0);
    assert_eq!(summary.created_dirs, 1);
    requests.assert_calls(0);
}

#[tokio::test]
async fn test_execute_create_dir_preserves_existing_local_file() {
    // 创建临时目录，并放入名为 `conflict` 的文件。
    let dir = tempdir().unwrap();
    let conflict_path = dir.path().join("conflict");
    std::fs::write(&conflict_path, b"I am a file").unwrap();

    assert!(
        conflict_path.is_file(),
        "Precondition: conflict should be a file"
    );

    let actions = vec![SyncAction::CreateDir {
        rel_path: "conflict".to_string(),
        target: SyncTarget::Local,
        target_abs: conflict_path.to_string_lossy().to_string(),
    }];

    let summary = execute_sync_actions(
        &Config::default(),
        actions,
        SyncExecuteOptions {
            dry_run: false,
            jobs: 1,

            print_actions: false,
        },
    )
    .await
    .unwrap();

    assert_eq!(std::fs::read(&conflict_path).unwrap(), b"I am a file");
    assert_eq!(summary.created_dirs, 0);
    assert_eq!(summary.failed, 1);
}

#[tokio::test]
async fn sync_executor_rejects_invalid_concurrency_before_execution() {
    let config = Config::default();
    let options = SyncExecuteOptions {
        dry_run: true,
        jobs: 32,

        print_actions: false,
    };
    execute_sync_actions(&config, vec![], options)
        .await
        .unwrap();

    for jobs in [0, 33] {
        let invalid = SyncExecuteOptions { jobs, ..options };
        let error = execute_sync_actions(&config, vec![], invalid)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("1 和 32"));
        let error = SyncRequest::new(
            "local",
            "cloud:/",
            SyncScanOptions {
                recursive: true,
                exclude: Vec::new(),
            },
            invalid,
            false,
            false,
        )
        .err()
        .expect("无效并发数不应创建同步请求");
        assert!(error.to_string().contains("1 和 32"));
    }
}
