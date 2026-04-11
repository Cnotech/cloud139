#![allow(dead_code)]

use cloud139::client::api::{HttpClientWrapper, get_personal_cloud_host_with_client};
use cloud139::config::Config;

#[tokio::test]
async fn test_get_personal_cloud_host_with_cached_host() {
    let mut config = Config::default();
    config.personal_cloud_host = Some("https://cached.example.com".to_string());

    let wrapper = HttpClientWrapper::new();
    let result = get_personal_cloud_host_with_client(&mut config, &wrapper).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "https://cached.example.com".to_string());
}

#[tokio::test]
async fn test_get_personal_cloud_host_with_empty_account() {
    let mut config = Config::default();
    config.account = "".to_string();
    config.authorization = "test".to_string();

    let wrapper = HttpClientWrapper::new();
    let result = get_personal_cloud_host_with_client(&mut config, &wrapper).await;

    assert!(result.is_err());
}
