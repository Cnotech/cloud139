#![allow(dead_code)]

use crate::cli::{CpArgs, DeleteArgs, MkdirArgs, MvArgs, RenameArgs, UploadArgs};

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

#[test]
fn test_mkdir_args_defaults() {
    let args = MkdirArgs {
        parents: false,
        path: "/test".to_string(),
    };
    assert_eq!(args.path, "/test");
}

#[test]
fn test_delete_args_validation() {
    let args = DeleteArgs {
        recursive: false,
        path: "/test.txt".to_string(),
        yes: false,
    };
    assert_eq!(args.path, "/test.txt");
    assert!(!args.yes);
}

#[test]
fn test_delete_args_with_yes() {
    let args = DeleteArgs {
        recursive: false,
        path: "/test.txt".to_string(),
        yes: true,
    };
    assert!(args.yes);
}

#[test]
fn test_delete_args_root_path() {
    let args = DeleteArgs {
        recursive: false,
        path: "/".to_string(),
        yes: true,
    };
    assert_eq!(args.path, "/");
}

#[test]
fn test_mv_args_validation_empty_source() {
    let args = MvArgs {
        source: vec![],
        target: "/".to_string(),
        force: false,
    };
    assert!(args.source.is_empty());
}

#[test]
fn test_mv_args_validation_single_source() {
    let args = MvArgs {
        source: vec!["/test.txt".to_string()],
        target: "/".to_string(),
        force: false,
    };
    assert_eq!(args.source.len(), 1);
}

#[test]
fn test_mv_args_validation_multiple_sources() {
    let args = MvArgs {
        source: vec!["/test1.txt".to_string(), "/test2.txt".to_string()],
        target: "/".to_string(),
        force: false,
    };
    assert_eq!(args.source.len(), 2);
}

#[test]
fn test_mv_args_with_force() {
    let args = MvArgs {
        source: vec!["/test.txt".to_string()],
        target: "/".to_string(),
        force: true,
    };
    assert!(args.force);
}
