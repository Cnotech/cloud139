#![allow(dead_code)]

use cloud139::services::mkdir_service;
use cloud139::commands::mkdir;

#[test]
fn test_mkdir_args_defaults() {
    let args = mkdir::MkdirArgs {
        path: "/test".to_string(),
        force: false,
    };
    assert_eq!(args.path, "/test");
    assert!(!args.force);
}

#[test]
fn test_mkdir_args_with_force() {
    let args = mkdir::MkdirArgs {
        path: "/test".to_string(),
        force: true,
    };
    assert!(args.force);
}

#[test]
fn test_parse_path_single() {
    let result = mkdir_service::parse_path("test");
    assert!(result.is_ok());
    let (parent, name) = result.unwrap();
    assert_eq!(parent, "/");
    assert_eq!(name, "test");
}

#[test]
fn test_parse_path_root() {
    let result = mkdir_service::parse_path("/test");
    assert!(result.is_ok());
    let (parent, name) = result.unwrap();
    assert_eq!(parent, "/");
    assert_eq!(name, "test");
}

#[test]
fn test_parse_path_nested() {
    let result = mkdir_service::parse_path("/parent/child");
    assert!(result.is_ok());
    let (parent, name) = result.unwrap();
    assert_eq!(parent, "/parent");
    assert_eq!(name, "child");
}

#[test]
fn test_parse_path_deep_nested() {
    let result = mkdir_service::parse_path("/a/b/c/d");
    assert!(result.is_ok());
    let (parent, name) = result.unwrap();
    assert_eq!(parent, "/a/b/c");
    assert_eq!(name, "d");
}

#[test]
fn test_parse_path_empty() {
    let result = mkdir_service::parse_path("");
    assert!(result.is_err());
}

#[test]
fn test_parse_path_only_slash() {
    let result = mkdir_service::parse_path("/");
    assert!(result.is_err());
}

#[test]
fn test_parse_path_whitespace() {
    let result = mkdir_service::parse_path("  ");
    assert!(result.is_err());
}

#[test]
fn test_parse_path_with_spaces() {
    let result = mkdir_service::parse_path("/my folder/file");
    assert!(result.is_ok());
    let (parent, name) = result.unwrap();
    assert_eq!(parent, "/my folder");
    assert_eq!(name, "file");
}
