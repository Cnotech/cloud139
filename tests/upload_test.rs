#![allow(dead_code)]

use cloud139::commands::upload;

#[test]
fn test_upload_args_defaults() {
    let args = upload::UploadArgs {
        local_path: "/local/file.txt".to_string(),
        remote_path: "/".to_string(),
        force: false,
    };
    assert_eq!(args.local_path, "/local/file.txt");
    assert_eq!(args.remote_path, "/");
    assert!(!args.force);
}

#[test]
fn test_upload_args_with_force() {
    let args = upload::UploadArgs {
        local_path: "/local/file.txt".to_string(),
        remote_path: "/remote/".to_string(),
        force: true,
    };
    assert!(args.force);
}

#[test]
fn test_upload_args_default_remote() {
    let args = upload::UploadArgs {
        local_path: "/local/file.txt".to_string(),
        remote_path: "/".to_string(),
        force: false,
    };
    assert_eq!(args.remote_path, "/");
}

#[test]
fn test_get_part_size_custom() {
    assert_eq!(upload::get_part_size(1024, 1024 * 1024), 1024 * 1024);
    assert_eq!(
        upload::get_part_size(1024 * 1024 * 1024, 50 * 1024 * 1024),
        50 * 1024 * 1024
    );
}

#[test]
fn test_get_part_size_30gb() {
    let size_30gb: i64 = 30 * 1024 * 1024 * 1024;
    assert_eq!(upload::get_part_size(size_30gb, 0), 100 * 1024 * 1024);
}

#[test]
fn test_get_part_size_over_30gb() {
    let size_31gb: i64 = 31 * 1024 * 1024 * 1024;
    assert_eq!(upload::get_part_size(size_31gb, 0), 512 * 1024 * 1024);
}

#[test]
fn test_get_part_size_100gb() {
    let size_100gb: i64 = 100 * 1024 * 1024 * 1024;
    assert_eq!(upload::get_part_size(size_100gb, 0), 512 * 1024 * 1024);
}

#[test]
fn test_get_part_size_small_file() {
    assert_eq!(
        upload::get_part_size(1024 * 1024 * 10, 0),
        100 * 1024 * 1024
    );
    assert_eq!(
        upload::get_part_size(1024 * 1024 * 20, 0),
        100 * 1024 * 1024
    );
    assert_eq!(
        upload::get_part_size(1024 * 1024 * 30, 0),
        100 * 1024 * 1024
    );
}

#[test]
fn test_upload_progress_bar_can_track_uploaded_bytes() {
    let pb = indicatif::ProgressBar::hidden();
    pb.set_length(5);
    pb.inc(2);
    pb.inc(3);
    assert_eq!(pb.position(), 5);
}
