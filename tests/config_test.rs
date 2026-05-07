use cloud139::config::{self, Config};
use cloud139::domain::StorageType;
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
        storage_type: "personal".to_string(),
        cloud_id: Some("cloud123".to_string()),
        custom_upload_part_size: 1024,
        report_real_size: true,
        use_large_thumbnail: false,
        personal_cloud_host: Some("host.example.com".to_string()),
        refresh_token: Some("token123".to_string()),
        token_expire_time: Some(1234567890000i64),
        root_folder_id: Some("folder123".to_string()),
        user_domain_id: Some("domain123".to_string()),
    };

    let content = toml::to_string_pretty(&config).unwrap();
    fs::write(&config_path, content).unwrap();

    let loaded: Config = toml::from_str(&fs::read_to_string(&config_path).unwrap()).unwrap();

    assert_eq!(loaded.authorization, "test_auth");
    assert_eq!(loaded.account, "test_account");
    assert_eq!(loaded.storage_type, "personal");
    assert_eq!(loaded.cloud_id, Some("cloud123".to_string()));
    assert_eq!(loaded.custom_upload_part_size, 1024);
    assert_eq!(loaded.report_real_size, true);
    assert_eq!(loaded.use_large_thumbnail, false);
    assert_eq!(
        loaded.personal_cloud_host,
        Some("host.example.com".to_string())
    );
    assert_eq!(loaded.refresh_token, Some("token123".to_string()));
    assert_eq!(loaded.token_expire_time, Some(1234567890000i64));
    assert_eq!(loaded.root_folder_id, Some("folder123".to_string()));
    assert_eq!(loaded.user_domain_id, Some("domain123".to_string()));
}

#[test]
fn test_config_storage_type_personal() {
    let config = Config {
        storage_type: "personal".to_string(),
        ..Default::default()
    };
    assert_eq!(config.storage_type(), StorageType::PersonalNew);
}

#[test]
fn test_config_storage_type_family() {
    let config = Config {
        storage_type: "family".to_string(),
        ..Default::default()
    };
    assert_eq!(config.storage_type(), StorageType::Family);
}

#[test]
fn test_config_storage_type_group() {
    let config = Config {
        storage_type: "group".to_string(),
        ..Default::default()
    };
    assert_eq!(config.storage_type(), StorageType::Group);
}

#[test]
fn test_config_storage_type_default() {
    let config = Config {
        storage_type: "".to_string(),
        ..Default::default()
    };
    assert_eq!(config.storage_type(), StorageType::PersonalNew);
}

#[test]
fn test_config_is_token_expired_no_time() {
    let config = Config {
        token_expire_time: None,
        ..Default::default()
    };
    assert!(config.is_token_expired());
}

#[test]
fn test_config_is_token_expired_expired() {
    let past_time = chrono::Utc::now().timestamp_millis() - 24 * 60 * 60 * 1000;
    let config = Config {
        token_expire_time: Some(past_time),
        ..Default::default()
    };
    assert!(config.is_token_expired());
}

#[test]
fn test_config_is_token_expired_not_expired() {
    let future_time = chrono::Utc::now().timestamp_millis() + 30 * 24 * 60 * 60 * 1000;
    let config = Config {
        token_expire_time: Some(future_time),
        ..Default::default()
    };
    assert!(!config.is_token_expired());
}

#[test]
fn test_config_is_token_expired_near_expiry() {
    let near_time = chrono::Utc::now().timestamp_millis() + 10 * 24 * 60 * 60 * 1000;
    let config = Config {
        token_expire_time: Some(near_time),
        ..Default::default()
    };
    assert!(config.is_token_expired());
}

#[test]
fn test_default_values() {
    let config = Config::default();
    assert_eq!(config.authorization, "");
    assert_eq!(config.account, "");
    assert_eq!(config.storage_type, "");
    assert_eq!(config.cloud_id, None);
    assert_eq!(config.custom_upload_part_size, 0);
    assert_eq!(config.report_real_size, true);
    assert_eq!(config.use_large_thumbnail, false);
    assert_eq!(config.personal_cloud_host, None);
    assert_eq!(config.refresh_token, None);
    assert_eq!(config.token_expire_time, None);
    assert_eq!(config.root_folder_id, None);
    assert_eq!(config.user_domain_id, None);
}

#[test]
fn test_config_path_default() {
    let path = Config::config_path();
    assert!(path.ends_with(config::CONFIG_FILE));
}

#[test]
fn test_global_config_path_contains_config_dir() {
    let path = config::global_config_path();
    assert!(path.ends_with(std::path::Path::new("cloud139").join("cloud139rc.toml")));
}
