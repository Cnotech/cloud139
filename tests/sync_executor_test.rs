use cloud139::config::Config;
use cloud139::domain::{ChangeKind, SyncAction, SyncSummary, SyncTarget};
use cloud139::services::sync_executor::{SyncExecuteOptions, execute_sync_actions};
use tempfile::tempdir;

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
            progress: false,
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
            progress: false,
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
    // Setup: create a temp dir with a file named "conflict"
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
            progress: false,
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
        progress: false,
        print_actions: false,
    };
    execute_sync_actions(&config, vec![], options)
        .await
        .unwrap();

    for jobs in [0, 33] {
        let error = execute_sync_actions(&config, vec![], SyncExecuteOptions { jobs, ..options })
            .await
            .unwrap_err();
        assert!(error.to_string().contains("1 和 32"));
    }
}
