#![allow(dead_code)]

use cloud139::commands::upload;

#[test]
fn test_get_part_size_custom() {
    let size = 1000;
    let custom = 512 * 1024 * 1024;
    let result = upload::get_part_size(size, custom);
    assert_eq!(result, custom);
}

#[test]
fn test_get_part_size_small_file() {
    let size = 1024 * 1024; // 1MB
    let custom = 0;
    let result = upload::get_part_size(size, custom);
    assert_eq!(result, 100 * 1024 * 1024); // default 100MB
}

#[test]
fn test_get_part_size_large_file() {
    let size = 31 * 1024 * 1024 * 1024; // 31GB
    let custom = 0;
    let result = upload::get_part_size(size, custom);
    assert_eq!(result, 512 * 1024 * 1024); // 512MB for files > 30GB
}

#[test]
fn test_get_part_size_boundary_30gb() {
    let size = 30 * 1024 * 1024 * 1024; // exactly 30GB
    let custom = 0;
    let result = upload::get_part_size(size, custom);
    assert_eq!(result, 100 * 1024 * 1024); // still default for 30GB
}

#[test]
fn test_get_part_size_just_over_30gb() {
    let size = 31 * 1024 * 1024 * 1024; // 31GB
    let custom = 0;
    let result = upload::get_part_size(size, custom);
    assert_eq!(result, 512 * 1024 * 1024); // 512MB for > 30GB
}

#[test]
fn test_upload_args_default() {
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
    assert_eq!(args.remote_path, "/remote/");
}
