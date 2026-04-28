use cloud139::services::sync_service::{
    compute_diff, format_action_line, SyncDiffOptions,
};
use cloud139::domain::{
    ChangeKind, FileEntry, SyncAction, SyncDirection, SyncEntryKind, SyncTarget,
};

fn entry(path: &str, size: u64, mtime: i64) -> FileEntry {
    FileEntry {
        rel_path: path.to_string(),
        size,
        mtime: Some(mtime),
        kind: SyncEntryKind::File,
    }
}

fn entry_no_mtime(path: &str, size: u64) -> FileEntry {
    FileEntry {
        rel_path: path.to_string(),
        size,
        mtime: None,
        kind: SyncEntryKind::File,
    }
}

fn dir_entry(path: &str) -> FileEntry {
    FileEntry {
        rel_path: path.to_string(),
        size: 0,
        mtime: None,
        kind: SyncEntryKind::Directory,
    }
}

#[test]
fn test_compute_diff_uploads_missing_target() {
    let actions = compute_diff(
        &[entry("docs/readme.md", 10, 100)],
        &[],
        SyncDiffOptions {
            direction: SyncDirection::LocalToCloud,
            delete: false,
            local_root: std::path::PathBuf::from("local"),
            cloud_root: "/remote".to_string(),
        },
    );

    assert_eq!(actions.len(), 1);
    assert!(
        matches!(actions[0], SyncAction::Upload { ref rel_path, .. } if rel_path == "docs/readme.md")
    );
    assert_eq!(
        format_action_line(&actions[0], false),
        ">f+++++++++ docs/readme.md"
    );
}

#[test]
fn test_compute_diff_skips_equal_size_and_mtime_with_two_second_tolerance() {
    let actions = compute_diff(
        &[entry("same.txt", 42, 100)],
        &[entry("same.txt", 42, 101)],
        SyncDiffOptions {
            direction: SyncDirection::LocalToCloud,
            delete: false,
            local_root: std::path::PathBuf::from("local"),
            cloud_root: "/remote".to_string(),
        },
    );

    assert_eq!(
        actions,
        vec![SyncAction::Skip {
            rel_path: "same.txt".to_string()
        }]
    );
}

#[test]
fn test_local_dir_vs_cloud_file_creates_dir_not_skip() {
    // Local has empty directory "conflict", cloud has 0-byte file "conflict"
    // This should NOT be treated as skip - it's a type conflict
    let actions = compute_diff(
        &[dir_entry("conflict")],
        &[entry("conflict", 0, 100)],
        SyncDiffOptions {
            direction: SyncDirection::LocalToCloud,
            delete: false,
            local_root: std::path::PathBuf::from("local"),
            cloud_root: "/remote".to_string(),
        },
    );

    // Should create directory, not skip
    assert!(
        actions
            .iter()
            .any(|a| matches!(a, SyncAction::CreateDir { rel_path, .. } if rel_path == "conflict")),
        "Expected CreateDir action for local dir vs cloud file, got: {:?}",
        actions
    );
}

#[test]
fn test_local_file_vs_cloud_dir_uploads_file_not_skip() {
    // Local has 0-byte file "conflict", cloud has empty directory "conflict"
    // This should NOT be treated as skip - it's a type conflict
    let actions = compute_diff(
        &[entry("conflict", 0, 100)],
        &[dir_entry("conflict")],
        SyncDiffOptions {
            direction: SyncDirection::LocalToCloud,
            delete: false,
            local_root: std::path::PathBuf::from("local"),
            cloud_root: "/remote".to_string(),
        },
    );

    // Should upload file, not skip
    assert!(
        actions
            .iter()
            .any(|a| matches!(a, SyncAction::Upload { rel_path, .. } if rel_path == "conflict")),
        "Expected Upload action for local file vs cloud dir, got: {:?}",
        actions
    );
}

#[test]
fn test_existing_directory_on_both_sides_produces_skip() {
    let actions = compute_diff(
        &[dir_entry("docs")],
        &[dir_entry("docs")],
        SyncDiffOptions {
            direction: SyncDirection::LocalToCloud,
            delete: false,
            local_root: std::path::PathBuf::from("local"),
            cloud_root: "/remote".to_string(),
        },
    );

    assert_eq!(
        actions,
        vec![SyncAction::Skip {
            rel_path: "docs".to_string()
        }]
    );
}

#[test]
fn test_compute_diff_creates_missing_directory_before_files() {
    let actions = compute_diff(
        &[dir_entry("empty"), entry("empty/file.txt", 4, 100)],
        &[],
        SyncDiffOptions {
            direction: SyncDirection::LocalToCloud,
            delete: false,
            local_root: std::path::PathBuf::from("local"),
            cloud_root: "/remote".to_string(),
        },
    );

    assert!(
        matches!(actions[0], SyncAction::CreateDir { ref rel_path, .. } if rel_path == "empty")
    );
    assert!(
        matches!(actions[1], SyncAction::Upload { ref rel_path, .. } if rel_path == "empty/file.txt")
    );
}

#[test]
fn test_compute_diff_deletes_empty_directory_after_children() {
    let actions = compute_diff(
        &[],
        &[dir_entry("old"), entry("old/file.txt", 4, 100)],
        SyncDiffOptions {
            direction: SyncDirection::CloudToLocal,
            delete: true,
            local_root: std::path::PathBuf::from("local"),
            cloud_root: "/remote".to_string(),
        },
    );

    assert!(
        matches!(actions[0], SyncAction::Delete { ref rel_path, .. } if rel_path == "old/file.txt")
    );
    assert!(matches!(actions[1], SyncAction::Delete { ref rel_path, .. } if rel_path == "old"));
}

#[test]
fn test_both_mtime_none_equal_size_is_skip() {
    let actions = compute_diff(
        &[entry_no_mtime("file.txt", 42)],
        &[entry_no_mtime("file.txt", 42)],
        SyncDiffOptions {
            direction: SyncDirection::LocalToCloud,
            delete: false,
            local_root: std::path::PathBuf::from("local"),
            cloud_root: "/remote".to_string(),
        },
    );

    assert_eq!(
        actions,
        vec![SyncAction::Skip {
            rel_path: "file.txt".to_string()
        }]
    );
}

#[test]
fn test_one_mtime_none_still_detects_size_change() {
    let actions = compute_diff(
        &[entry_no_mtime("file.txt", 100)],
        &[entry("file.txt", 42, 101)],
        SyncDiffOptions {
            direction: SyncDirection::LocalToCloud,
            delete: false,
            local_root: std::path::PathBuf::from("local"),
            cloud_root: "/remote".to_string(),
        },
    );

    assert_eq!(actions.len(), 1);
    assert!(matches!(
        &actions[0],
        SyncAction::Upload { rel_path, change: ChangeKind::SizeOrTime, .. }
        if rel_path == "file.txt"
    ));
}

#[test]
fn test_compute_diff_transfers_when_mtime_diff_is_larger_than_two_seconds() {
    let actions = compute_diff(
        &[entry("changed.txt", 42, 100)],
        &[entry("changed.txt", 42, 103)],
        SyncDiffOptions {
            direction: SyncDirection::CloudToLocal,
            delete: false,
            local_root: std::path::PathBuf::from("local"),
            cloud_root: "/remote".to_string(),
        },
    );

    assert!(
        matches!(actions[0], SyncAction::Download { ref rel_path, .. } if rel_path == "changed.txt")
    );
    assert_eq!(
        format_action_line(&actions[0], false),
        ">f.st...... changed.txt"
    );
}

#[test]
fn test_compute_diff_adds_delete_actions_for_extra_target_files() {
    let actions = compute_diff(
        &[entry("keep.txt", 1, 1)],
        &[entry("keep.txt", 1, 1), entry("old.txt", 1, 1)],
        SyncDiffOptions {
            direction: SyncDirection::LocalToCloud,
            delete: true,
            local_root: std::path::PathBuf::from("local"),
            cloud_root: "/remote".to_string(),
        },
    );

    assert!(actions.contains(&SyncAction::Delete {
        rel_path: "old.txt".to_string(),
        target: SyncTarget::Cloud,
        target_abs: "/remote/old.txt".to_string(),
    }));
}

#[test]
fn test_local_to_cloud_skips_on_equal_size_ignores_mtime() {
    let actions = compute_diff(
        &[entry("file.txt", 42, 100)],
        &[entry("file.txt", 42, 999)],
        SyncDiffOptions {
            direction: SyncDirection::LocalToCloud,
            delete: false,
            local_root: std::path::PathBuf::from("local"),
            cloud_root: "/remote".to_string(),
        },
    );

    assert_eq!(
        actions,
        vec![SyncAction::Skip {
            rel_path: "file.txt".to_string()
        }]
    );
}

#[test]
fn test_local_to_cloud_detects_size_change() {
    let actions = compute_diff(
        &[entry("file.txt", 100, 100)],
        &[entry("file.txt", 42, 100)],
        SyncDiffOptions {
            direction: SyncDirection::LocalToCloud,
            delete: false,
            local_root: std::path::PathBuf::from("local"),
            cloud_root: "/remote".to_string(),
        },
    );

    assert_eq!(actions.len(), 1);
    assert!(matches!(
        &actions[0],
        SyncAction::Upload { rel_path, change: ChangeKind::SizeOrTime, .. }
        if rel_path == "file.txt"
    ));
}

#[test]
fn test_cloud_to_local_detects_mtime_change() {
    let actions = compute_diff(
        &[entry("file.txt", 42, 100)],
        &[entry("file.txt", 42, 105)],
        SyncDiffOptions {
            direction: SyncDirection::CloudToLocal,
            delete: false,
            local_root: std::path::PathBuf::from("local"),
            cloud_root: "/remote".to_string(),
        },
    );

    assert_eq!(actions.len(), 1);
    assert!(matches!(
        &actions[0],
        SyncAction::Download { rel_path, change: ChangeKind::SizeOrTime, .. }
        if rel_path == "file.txt"
    ));
}

#[test]
fn test_cloud_to_local_skips_on_equal_size_and_mtime() {
    let actions = compute_diff(
        &[entry("file.txt", 42, 100)],
        &[entry("file.txt", 42, 101)],
        SyncDiffOptions {
            direction: SyncDirection::CloudToLocal,
            delete: false,
            local_root: std::path::PathBuf::from("local"),
            cloud_root: "/remote".to_string(),
        },
    );

    assert_eq!(
        actions,
        vec![SyncAction::Skip {
            rel_path: "file.txt".to_string()
        }]
    );
}
