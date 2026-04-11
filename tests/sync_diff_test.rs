use cloud139::application::services::sync_service::{
    SyncDiffOptions, compute_diff, format_action_line,
};
use cloud139::domain::{ChangeKind, FileEntry, SyncAction, SyncDirection, SyncTarget};

fn entry(path: &str, size: u64, mtime: i64, checksum: Option<&str>) -> FileEntry {
    FileEntry {
        rel_path: path.to_string(),
        size,
        mtime: Some(mtime),
        checksum: checksum.map(str::to_string),
    }
}

fn entry_no_mtime(path: &str, size: u64, checksum: Option<&str>) -> FileEntry {
    FileEntry {
        rel_path: path.to_string(),
        size,
        mtime: None,
        checksum: checksum.map(str::to_string),
    }
}

#[test]
fn test_compute_diff_uploads_missing_target() {
    let actions = compute_diff(
        &[entry("docs/readme.md", 10, 100, None)],
        &[],
        SyncDiffOptions {
            direction: SyncDirection::LocalToCloud,
            delete: false,
            checksum: false,
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
        &[entry("same.txt", 42, 100, None)],
        &[entry("same.txt", 42, 101, None)],
        SyncDiffOptions {
            direction: SyncDirection::LocalToCloud,
            delete: false,
            checksum: false,
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
fn test_both_mtime_none_equal_size_is_skip() {
    let actions = compute_diff(
        &[entry_no_mtime("file.txt", 42, None)],
        &[entry_no_mtime("file.txt", 42, None)],
        SyncDiffOptions {
            direction: SyncDirection::LocalToCloud,
            delete: false,
            checksum: false,
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
        &[entry_no_mtime("file.txt", 100, None)],
        &[entry("file.txt", 42, 101, None)],
        SyncDiffOptions {
            direction: SyncDirection::LocalToCloud,
            delete: false,
            checksum: false,
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
        &[entry("changed.txt", 42, 100, None)],
        &[entry("changed.txt", 42, 103, None)],
        SyncDiffOptions {
            direction: SyncDirection::CloudToLocal,
            delete: false,
            checksum: false,
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
fn test_compute_diff_uses_checksum_when_enabled() {
    let actions = compute_diff(
        &[entry("hash.txt", 42, 100, Some("aaa"))],
        &[entry("hash.txt", 42, 100, Some("bbb"))],
        SyncDiffOptions {
            direction: SyncDirection::LocalToCloud,
            delete: false,
            checksum: true,
            local_root: std::path::PathBuf::from("local"),
            cloud_root: "/remote".to_string(),
        },
    );

    assert!(
        matches!(actions[0], SyncAction::Upload { ref rel_path, .. } if rel_path == "hash.txt")
    );
    assert_eq!(
        format_action_line(&actions[0], true),
        "(DRY RUN) >f.c....... hash.txt"
    );
}

#[test]
fn test_compute_diff_adds_delete_actions_for_extra_target_files() {
    let actions = compute_diff(
        &[entry("keep.txt", 1, 1, None)],
        &[entry("keep.txt", 1, 1, None), entry("old.txt", 1, 1, None)],
        SyncDiffOptions {
            direction: SyncDirection::LocalToCloud,
            delete: true,
            checksum: false,
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
fn test_checksum_mode_falls_back_to_size_mtime_when_one_side_none() {
    let actions = compute_diff(
        &[entry("file.txt", 42, 100, Some("abc123"))],
        &[entry("file.txt", 42, 101, None)],
        SyncDiffOptions {
            direction: SyncDirection::LocalToCloud,
            delete: false,
            checksum: true,
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
fn test_checksum_mode_falls_back_detects_size_change() {
    let actions = compute_diff(
        &[entry("file.txt", 100, 100, Some("abc123"))],
        &[entry("file.txt", 42, 101, None)],
        SyncDiffOptions {
            direction: SyncDirection::CloudToLocal,
            delete: false,
            checksum: true,
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
fn test_checksum_mode_falls_back_detects_mtime_change() {
    let actions = compute_diff(
        &[entry("file.txt", 42, 100, Some("abc123"))],
        &[entry("file.txt", 42, 105, None)],
        SyncDiffOptions {
            direction: SyncDirection::LocalToCloud,
            delete: false,
            checksum: true,
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
fn test_checksum_mode_both_none_falls_back_to_size_mtime() {
    let actions = compute_diff(
        &[entry("file.txt", 42, 100, None)],
        &[entry("file.txt", 42, 101, None)],
        SyncDiffOptions {
            direction: SyncDirection::LocalToCloud,
            delete: false,
            checksum: true,
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
fn test_checksum_mode_detects_change_when_both_sides_have_checksum() {
    let actions = compute_diff(
        &[entry("file.txt", 42, 100, Some("abc123"))],
        &[entry("file.txt", 42, 100, Some("def456"))],
        SyncDiffOptions {
            direction: SyncDirection::LocalToCloud,
            delete: false,
            checksum: true,
            local_root: std::path::PathBuf::from("local"),
            cloud_root: "/remote".to_string(),
        },
    );

    assert_eq!(actions.len(), 1);
    assert!(matches!(
        &actions[0],
        SyncAction::Upload { rel_path, change: ChangeKind::Checksum, .. }
        if rel_path == "file.txt"
    ));
}

#[test]
fn test_checksum_mode_skips_when_both_checksums_match() {
    let actions = compute_diff(
        &[entry("file.txt", 42, 100, Some("abc123"))],
        &[entry("file.txt", 42, 100, Some("abc123"))],
        SyncDiffOptions {
            direction: SyncDirection::LocalToCloud,
            delete: false,
            checksum: true,
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
