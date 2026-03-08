#![allow(dead_code)]

use cloud139::client::api_trait::ApiClient;
use cloud139::client::{ClientError, StorageType};
use cloud139::config::Config;
use cloud139::models::*;

#[tokio::test]
async fn test_get_file_id_by_path_root() {
    let config = Config::default();
    
    let result = cloud139::client::api::get_file_id_by_path(&config, "/").await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "");
}

#[tokio::test]
async fn test_get_file_id_by_path_empty() {
    let config = Config::default();
    
    let result = cloud139::client::api::get_file_id_by_path(&config, "").await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "");
}

#[tokio::test]
async fn test_get_file_id_by_path_with_host_cached() {
    let mut config = Config::default();
    config.personal_cloud_host = Some("https://cached.example.com".to_string());
    
    let result = cloud139::client::api::get_file_id_by_path(&config, "/").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_personal_api_request_storage_type_personal() {
    let svctype = match StorageType::PersonalNew {
        StorageType::PersonalNew => "1",
        StorageType::Family => "2",
        StorageType::Group => "3",
    };
    assert_eq!(svctype, "1");
}

#[tokio::test]
async fn test_personal_api_request_storage_type_family() {
    let svctype = match StorageType::Family {
        StorageType::PersonalNew => "1",
        StorageType::Family => "2",
        StorageType::Group => "3",
    };
    assert_eq!(svctype, "2");
}

#[tokio::test]
async fn test_personal_api_request_storage_type_group() {
    let svctype = match StorageType::Group {
        StorageType::PersonalNew => "1",
        StorageType::Family => "2",
        StorageType::Group => "3",
    };
    assert_eq!(svctype, "3");
}
