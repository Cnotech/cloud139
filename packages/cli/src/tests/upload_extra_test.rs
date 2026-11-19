#![allow(dead_code)]

use crate::cli::UploadArgs;
use cloud139::client::api::upload::get_part_size;

#[test]
fn test_get_part_size_custom() {
    let size = 1000;
    let custom = 512 * 1024 * 1024;
    let result = get_part_size(size, custom);
    assert_eq!(result, custom);
}

#[test]
fn test_get_part_size_small_file() {
    let size = 1024 * 1024; // 文件大小为 1 MiB
    let custom = 0;
    let result = get_part_size(size, custom);
    assert_eq!(result, 100 * 1024 * 1024); // 默认分片大小为 100 MiB
}

#[test]
fn test_get_part_size_large_file() {
    let size = 31 * 1024 * 1024 * 1024; // 文件大小为 31 GiB
    let custom = 0;
    let result = get_part_size(size, custom);
    assert_eq!(result, 512 * 1024 * 1024); // 超过 30 GiB 的文件使用 512 MiB 分片
}

#[test]
fn test_get_part_size_boundary_30gb() {
    let size = 30 * 1024 * 1024 * 1024; // 文件大小恰好为 30 GiB
    let custom = 0;
    let result = get_part_size(size, custom);
    assert_eq!(result, 100 * 1024 * 1024); // 恰好为 30 GiB 时仍使用默认分片大小
}

#[test]
fn test_get_part_size_just_over_30gb() {
    let size = 31 * 1024 * 1024 * 1024; // 文件大小为 31 GiB
    let custom = 0;
    let result = get_part_size(size, custom);
    assert_eq!(result, 512 * 1024 * 1024); // 超过 30 GiB 时使用 512 MiB 分片
}

#[test]
fn test_upload_args_default() {
    let args = UploadArgs {
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
    let args = UploadArgs {
        local_path: "/local/file.txt".to_string(),
        remote_path: "/remote/".to_string(),
        force: true,
    };
    assert!(args.force);
    assert_eq!(args.remote_path, "/remote/");
}
