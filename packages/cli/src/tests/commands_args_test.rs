#![allow(dead_code)]

use crate::cli::args::{CpArgs, RenameArgs, UploadArgs};

#[test]
fn test_rename_args_validation() {
    let args = RenameArgs {
        source: "/old.txt".to_string(),
        target: "new.txt".to_string(),
        force: false,
    };
    assert_eq!(args.source, "/old.txt");
    assert_eq!(args.target, "new.txt");
}

#[test]
fn test_cp_args_validation() {
    let args = CpArgs {
        source: vec!["/source.txt".to_string()],
        target: "/target/".to_string(),
        force: false,
    };
    assert_eq!(args.source, vec!["/source.txt"]);
    assert_eq!(args.target, "/target/");
    assert!(!args.force);
}

#[test]
fn test_cp_args_with_force() {
    let args = CpArgs {
        source: vec!["/source.txt".to_string()],
        target: "/target/".to_string(),
        force: true,
    };
    assert!(args.force);
}

#[test]
fn test_upload_args_validation() {
    let args = UploadArgs {
        local_path: "/local/file.txt".to_string(),
        remote_path: "/remote/".to_string(),
        force: false,
    };
    assert_eq!(args.local_path, "/local/file.txt");
    assert_eq!(args.remote_path, "/remote/");
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
}

#[test]
fn test_upload_args_default_remote_path() {
    let args = UploadArgs {
        local_path: "/local/file.txt".to_string(),
        remote_path: "/".to_string(),
        force: false,
    };
    assert_eq!(args.remote_path, "/");
}
