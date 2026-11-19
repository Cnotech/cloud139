#![allow(dead_code)]

use cloud139::client::http::HttpClientWrapper;
use cloud139::config::Config;

#[tokio::test]
async fn test_get_file_id_by_path_root() {
    let config = Config::default();

    let result = cloud139::client::api::files::get_file_id_by_path(&config, "/").await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "");
}

#[tokio::test]
async fn test_get_file_id_by_path_empty() {
    let config = Config::default();

    let result = cloud139::client::api::files::get_file_id_by_path(&config, "").await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "");
}

#[tokio::test]
async fn test_get_file_id_by_path_with_host_cached() {
    let config = Config {
        personal_cloud_host: Some("https://cached.example.com".to_string()),
        ..Default::default()
    };

    let result = cloud139::client::api::files::get_file_id_by_path(&config, "/").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_get_personal_cloud_host_cached() {
    let mut config = Config {
        personal_cloud_host: Some("cached-host.example.com".to_string()),
        ..Default::default()
    };
    let http_client = HttpClientWrapper::new();
    let host = http_client.get_personal_cloud_host(&mut config).await;

    assert!(host.is_ok());
    assert_eq!(host.unwrap(), "cached-host.example.com");
}
