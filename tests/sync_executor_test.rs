use cloud139::application::services::sync_executor::{SyncExecuteOptions, execute_sync_actions};
use cloud139::config::Config;
use cloud139::domain::{ChangeKind, SyncAction, SyncSummary, SyncTarget};

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
