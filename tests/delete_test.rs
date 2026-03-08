#![allow(dead_code)]

use cloud139::commands::delete;

#[test]
fn test_delete_args_validation() {
    let args = delete::DeleteArgs {
        path: "/test.txt".to_string(),
        yes: false,
        permanent: false,
    };
    assert_eq!(args.path, "/test.txt");
    assert!(!args.yes);
    assert!(!args.permanent);
}

#[test]
fn test_delete_args_with_yes() {
    let args = delete::DeleteArgs {
        path: "/test.txt".to_string(),
        yes: true,
        permanent: false,
    };
    assert!(args.yes);
    assert!(!args.permanent);
}

#[test]
fn test_delete_args_permanent() {
    let args = delete::DeleteArgs {
        path: "/test.txt".to_string(),
        yes: true,
        permanent: true,
    };
    assert!(args.yes);
    assert!(args.permanent);
}

#[test]
fn test_delete_args_root_path() {
    let args = delete::DeleteArgs {
        path: "/".to_string(),
        yes: true,
        permanent: false,
    };
    assert_eq!(args.path, "/");
}
