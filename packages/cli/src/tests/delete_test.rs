#![allow(dead_code)]

use crate::cli::args::DeleteArgs;

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
