use crate::config;
use cloud139::config::Config;

use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

fn create_temp_dir() -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("cloud139rc.toml");
    (temp_dir, config_path)
}

#[test]
fn test_config_load_invalid_toml() {
    let (_temp_dir, config_path) = create_temp_dir();
    fs::write(&config_path, "invalid toml").unwrap();

    let result = fs::read_to_string(&config_path).unwrap();
    let config: Result<Config, _> = toml::from_str(&result);
    assert!(config.is_err());
}

#[test]
fn test_config_save_and_load() {
    let (_temp_dir, config_path) = create_temp_dir();

    let config = Config {
        authorization: "test_auth".to_string(),
        account: "test_account".to_string(),
        storage_type: "personal_new".to_string(),
        custom_upload_part_size: 1024,
        use_large_thumbnail: false,
        personal_cloud_host: Some("host.example.com".to_string()),
        refresh_token: Some("token123".to_string()),
        token_expire_time: Some(1234567890000i64),
    };

    let content = toml::to_string_pretty(&config).unwrap();
    fs::write(&config_path, content).unwrap();

    let loaded: Config = toml::from_str(&fs::read_to_string(&config_path).unwrap()).unwrap();

    assert_eq!(loaded.authorization, "test_auth");
    assert_eq!(loaded.account, "test_account");
    assert_eq!(loaded.storage_type, "personal_new");
    assert_eq!(loaded.custom_upload_part_size, 1024);
    assert_eq!(loaded.use_large_thumbnail, false);
    assert_eq!(
        loaded.personal_cloud_host,
        Some("host.example.com".to_string())
    );
    assert_eq!(loaded.refresh_token, Some("token123".to_string()));
    assert_eq!(loaded.token_expire_time, Some(1234567890000i64));
}

#[test]
fn test_token_refresh_due_no_time() {
    let config = Config {
        token_expire_time: None,
        ..Default::default()
    };
    assert!(config.is_token_refresh_due());
}

#[test]
fn test_token_refresh_due_expired() {
    let past_time = chrono::Utc::now().timestamp_millis() - 24 * 60 * 60 * 1000;
    let config = Config {
        token_expire_time: Some(past_time),
        ..Default::default()
    };
    assert!(config.is_token_refresh_due());
}

#[test]
fn test_token_refresh_due_not_expired() {
    let future_time = chrono::Utc::now().timestamp_millis() + 30 * 24 * 60 * 60 * 1000;
    let config = Config {
        token_expire_time: Some(future_time),
        ..Default::default()
    };
    assert!(!config.is_token_refresh_due());
}

#[test]
fn test_token_refresh_due_near_expiry() {
    let near_time = chrono::Utc::now().timestamp_millis() + 3 * 24 * 60 * 60 * 1000;
    let config = Config {
        token_expire_time: Some(near_time),
        ..Default::default()
    };
    assert!(config.is_token_refresh_due());
}

#[test]
fn test_default_values() {
    let config = Config::default();
    assert_eq!(config.authorization, "");
    assert_eq!(config.account, "");
    assert_eq!(config.storage_type, "personal_new");
    assert_eq!(config.custom_upload_part_size, 0);
    assert_eq!(config.use_large_thumbnail, false);
    assert_eq!(config.personal_cloud_host, None);
    assert_eq!(config.refresh_token, None);
    assert_eq!(config.token_expire_time, None);
}

#[test]
fn test_config_path_default() {
    let path = config::local_config_path();
    assert!(path.ends_with("cloud139rc.toml"));
}

#[test]
fn test_global_config_path_contains_config_dir() {
    let path = config::global_config_path();
    assert!(path.ends_with(std::path::Path::new("cloud139").join("cloud139rc.toml")));
}
