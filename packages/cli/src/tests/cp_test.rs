#![allow(dead_code)]

use crate::cli::CpArgs;
use cloud139::config::Config;
use cloud139::error::ClientError;
use cloud139::services::directory::cp;

#[test]
fn test_cp_args_validation_single_source() {
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
fn test_cp_args_validation_multiple_sources() {
    let args = CpArgs {
        source: vec!["/source1.txt".to_string(), "/source2.txt".to_string()],
        target: "/target/".to_string(),
        force: false,
    };
    assert_eq!(args.source.len(), 2);
    assert_eq!(args.source[0], "/source1.txt");
    assert_eq!(args.source[1], "/source2.txt");
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

#[tokio::test]
async fn test_cp_family_rejects_multiple_sources() {
    let config = Config {
        storage_type: "family".to_string(),
        ..Config::default()
    };
    let sources = vec!["/source1.txt".to_string(), "/source2.txt".to_string()];

    let result = cp(&config, &sources, "/target/", false).await;

    assert!(matches!(result, Err(ClientError::UnsupportedStorage)));
}

#[tokio::test]
async fn test_cp_group_rejects_multiple_sources() {
    let config = Config {
        storage_type: "group".to_string(),
        ..Config::default()
    };
    let sources = vec!["/source1.txt".to_string(), "/source2.txt".to_string()];

    let result = cp(&config, &sources, "/target/", false).await;

    assert!(matches!(result, Err(ClientError::UnsupportedStorage)));
}
