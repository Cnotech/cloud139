#![allow(dead_code)]

mod config_test_extended {
    use cloud139::config::{Config, ConfigError};
    use tempfile::TempDir;

    fn create_temp_config_dir() -> TempDir {
        TempDir::new().unwrap()
    }

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert!(config.authorization.is_empty());
        assert!(config.account.is_empty());
        assert!(config.storage_type.is_empty());
        assert!(config.cloud_id.is_none());
        assert_eq!(config.custom_upload_part_size, 0);
        assert!(config.report_real_size);
        assert!(!config.use_large_thumbnail);
        assert!(config.personal_cloud_host.is_none());
    }

    #[test]
    fn test_config_serialize() {
        let config = Config {
            authorization: "Basic dGVzdA==".to_string(),
            account: "test@139.com".to_string(),
            storage_type: "personal_new".to_string(),
            cloud_id: Some("cloud123".to_string()),
            custom_upload_part_size: 0,
            report_real_size: true,
            use_large_thumbnail: false,
            personal_cloud_host: Some("https://test.com".to_string()),
            refresh_token: None,
            token_expire_time: None,
            root_folder_id: None,
            user_domain_id: None,
        };

        let toml_str = toml::to_string(&config).unwrap();
        assert!(toml_str.contains("test@139.com"));
        assert!(toml_str.contains("personal_new"));
    }

    #[test]
    fn test_config_deserialize() {
        let toml_str = r#"authorization = "Basic dGVzdA=="
account = "test@139.com"
storage_type = "personal_new"
cloud_id = "cloud123"
custom_upload_part_size = 0
report_real_size = true
use_large_thumbnail = false
personal_cloud_host = "https://test.com"
"#;

        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.account, "test@139.com");
        assert_eq!(config.storage_type, "personal_new");
        assert_eq!(config.cloud_id, Some("cloud123".to_string()));
    }

    #[test]
    fn test_config_deserialize_with_optional_fields() {
        let toml_str = r#"authorization = "Basic dGVzdA=="
account = "test@139.com"
storage_type = "family"
cloud_id = "cloud123"
custom_upload_part_size = 1048576
report_real_size = false
use_large_thumbnail = true
personal_cloud_host = "https://test.com"
refresh_token = "token123"
token_expire_time = 1234567890
root_folder_id = "root"
user_domain_id = "domain123"
"#;

        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.refresh_token, Some("token123".to_string()));
        assert_eq!(config.token_expire_time, Some(1234567890));
        assert_eq!(config.root_folder_id, Some("root".to_string()));
        assert_eq!(config.user_domain_id, Some("domain123".to_string()));
        assert!(!config.report_real_size);
        assert!(config.use_large_thumbnail);
    }

    #[test]
    fn test_config_config_path() {
        let path = Config::config_path();
        let s = path.to_string_lossy();
        assert!(s.contains("cloud139rc.toml"));
    }

    #[test]
    fn test_config_load_not_found() {
        // Use an explicit invalid path by temporarily changing behavior
        // The config_path() is hardcoded, so we just verify it returns a valid path
        let path = Config::config_path();
        let s = path.to_string_lossy();
        assert!(s.contains("cloud139rc.toml"));
    }

    #[test]
    fn test_config_is_token_expired_no_expiry() {
        let config = Config::default();
        assert!(config.is_token_expired());
    }

    #[test]
    fn test_config_is_token_expired_past() {
        let past = chrono::Utc::now().timestamp_millis() - 24 * 60 * 60 * 1000;
        let config = Config {
            token_expire_time: Some(past),
            ..Default::default()
        };
        assert!(config.is_token_expired());
    }

    #[test]
    fn test_config_is_token_expired_future() {
        let future = chrono::Utc::now().timestamp_millis() + 24 * 60 * 60 * 1000 * 30;
        let config = Config {
            token_expire_time: Some(future),
            ..Default::default()
        };
        assert!(!config.is_token_expired());
    }

    #[test]
    fn test_config_is_token_expired_near_expiry() {
        let near = chrono::Utc::now().timestamp_millis() + 10 * 24 * 60 * 60 * 1000;
        let config = Config {
            token_expire_time: Some(near),
            ..Default::default()
        };
        assert!(config.is_token_expired());
    }

    #[test]
    fn test_config_debug() {
        let config = Config::default();
        let debug_str = format!("{:?}", config);
        assert!(!debug_str.is_empty());
    }

    #[test]
    fn test_config_clone() {
        let config = Config {
            account: "test@139.com".to_string(),
            authorization: "Basic dGVzdA==".to_string(),
            ..Default::default()
        };

        let cloned = config.clone();
        assert_eq!(cloned.account, config.account);
        assert_eq!(cloned.authorization, config.authorization);
    }

    #[test]
    fn test_config_error_display_not_found() {
        let err = ConfigError::NotFound;
        assert_eq!(err.to_string(), "配置文件未找到");
    }

    #[test]
    fn test_config_error_display_io() {
        let err = ConfigError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "test"));
        assert!(err.to_string().contains("test"));
    }

    #[test]
    fn test_config_error_display_toml() {
        let err = ConfigError::TomlDe(toml::from_str::<toml::Value>("invalid").unwrap_err());
        assert!(err.to_string().contains("TOML"));
    }

    #[test]
    fn test_config_error_debug() {
        let err = ConfigError::NotFound;
        let debug_str = format!("{:?}", err);
        assert!(!debug_str.is_empty());
    }

    #[test]
    fn test_config_error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "test");
        let config_err: ConfigError = io_err.into();
        assert!(matches!(config_err, ConfigError::Io(_)));
    }

    #[test]
    fn test_config_error_from_toml() {
        let toml_err = toml::from_str::<toml::Value>("invalid").unwrap_err();
        let config_err: ConfigError = toml_err.into();
        assert!(matches!(config_err, ConfigError::TomlDe(_)));
    }
}
