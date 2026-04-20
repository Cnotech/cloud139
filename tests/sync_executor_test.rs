use cloud139::application::services::sync_executor::{SyncExecuteOptions, execute_sync_actions};
use cloud139::config::Config;
use cloud139::domain::{ChangeKind, SyncAction, SyncSummary, SyncTarget};
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
async fn test_execute_create_dir_replaces_existing_local_file() {
    // Setup: create a temp dir with a file named "conflict"
    let dir = tempdir().unwrap();
    let conflict_path = dir.path().join("conflict");
    std::fs::write(&conflict_path, b"I am a file").unwrap();

    assert!(conflict_path.is_file(), "Precondition: conflict should be a file");

    // Action: CreateDir should replace the file with a directory
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

    // Verify: conflict should now be a directory
    assert!(conflict_path.is_dir(), "After CreateDir, conflict should be a directory");
    assert_eq!(summary.created_dirs, 1);
    assert_eq!(summary.failed, 0);
}
