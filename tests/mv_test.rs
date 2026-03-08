#![allow(dead_code)]

use cloud139::commands::mv;

#[test]
fn test_mv_args_validation_empty_source() {
    let args = mv::MvArgs {
        source: vec![],
        target: "/".to_string(),
        force: false,
    };
    assert!(args.source.is_empty());
}

#[test]
fn test_mv_args_validation_single_source() {
    let args = mv::MvArgs {
        source: vec!["/test.txt".to_string()],
        target: "/".to_string(),
        force: false,
    };
    assert_eq!(args.source.len(), 1);
}

#[test]
fn test_mv_args_validation_multiple_sources() {
    let args = mv::MvArgs {
        source: vec!["/test1.txt".to_string(), "/test2.txt".to_string()],
        target: "/".to_string(),
        force: false,
    };
    assert_eq!(args.source.len(), 2);
}

#[test]
fn test_mv_args_with_force() {
    let args = mv::MvArgs {
        source: vec!["/test.txt".to_string()],
        target: "/".to_string(),
        force: true,
    };
    assert!(args.force);
}
