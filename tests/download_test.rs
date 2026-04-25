#![allow(dead_code)]

use cloud139::commands::download;
use cloud139::utils::resolve_local_path;

#[test]
fn test_resolve_local_path_none() {
    let remote_path = "/test/file.txt";
    let local_path = None;
    let result = resolve_local_path(remote_path, &local_path);
    assert_eq!(result, "file.txt");
}

#[test]
fn test_resolve_local_path_some_with_file() {
    let remote_path = "/test/file.txt";
    let local_path = Some("/local/path.txt".to_string());
    let result = resolve_local_path(remote_path, &local_path);
    assert_eq!(result, "/local/path.txt");
}

#[test]
fn test_resolve_local_path_with_directory() {
    let remote_path = "/test/file.txt";
    let local_path = Some("/local/dir/".to_string());
    let result = resolve_local_path(remote_path, &local_path);
    assert_eq!(result, "/local/dir/file.txt");
}

#[test]
fn test_resolve_local_path_with_dir_no_slash() {
    let remote_path = "/test/file.txt";
    let local_path = Some("/local/dir".to_string());
    let result = resolve_local_path(remote_path, &local_path);
    assert_eq!(result, "/local/dir");
}

#[test]
fn test_resolve_local_path_no_extension() {
    let remote_path = "/test/myfile";
    let local_path = Some("/local/dir".to_string());
    let result = resolve_local_path(remote_path, &local_path);
    assert_eq!(result, "/local/dir");
}

#[test]
fn test_resolve_local_path_empty_remote() {
    let remote_path = "";
    let local_path = None;
    let result = resolve_local_path(remote_path, &local_path);
    assert_eq!(result, "download");
}

#[test]
fn test_resolve_local_path_only_slash() {
    let remote_path = "/";
    let local_path = None;
    let result = resolve_local_path(remote_path, &local_path);
    assert_eq!(result, "download");
}

#[test]
fn test_resolve_local_path_with_txt_extension() {
    let remote_path = "/test/file";
    let local_path = Some("/local/dir/file.txt".to_string());
    let result = resolve_local_path(remote_path, &local_path);
    assert_eq!(result, "/local/dir/file.txt");
}

#[test]
fn test_resolve_local_path_nested() {
    let remote_path = "/a/b/c/d/file.txt";
    let local_path = None;
    let result = resolve_local_path(remote_path, &local_path);
    assert_eq!(result, "file.txt");
}

#[test]
fn test_resolve_local_path_target_is_dir() {
    let remote_path = "/test/file.txt";
    let local_path = Some("/local/dir/".to_string());
    let result = resolve_local_path(remote_path, &local_path);
    assert_eq!(result, "/local/dir/file.txt");
}

#[test]
fn test_download_args_default() {
    let args = download::DownloadArgs {
        remote_path: "/test.txt".to_string(),
        local_path: None,
    };
    assert_eq!(args.remote_path, "/test.txt");
    assert_eq!(args.local_path, None);
}

#[test]
fn test_download_args_with_local_path() {
    let args = download::DownloadArgs {
        remote_path: "/test.txt".to_string(),
        local_path: Some("/local/path.txt".to_string()),
    };
    assert_eq!(args.local_path, Some("/local/path.txt".to_string()));
}

#[test]
fn test_download_progress_style_can_be_constructed() {
    let pb = indicatif::ProgressBar::hidden();
    pb.set_length(10);
    pb.inc(4);
    assert_eq!(pb.position(), 4);
}
