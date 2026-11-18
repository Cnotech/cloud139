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

#[tokio::test]
async fn cli_and_library_post_entry_points_send_the_same_personal_headers() {
    use cloud139::{
        client::{Client, PC_CLIENT_INFO, api::personal_api_request},
        domain::StorageType,
    };
    use httpmock::prelude::*;
    use serde_json::{Value, json};

    let server = MockServer::start();
    let request = server.mock(|when, then| {
        when.method(POST)
            .path("/post")
            .header("content-type", "application/json")
            .header("x-yun-client-info", PC_CLIENT_INFO)
            .header("authorization", "Basic fixture")
            .json_body(json!({"key":"中文"}));
        then.json_body(json!({"success":true}));
    });
    let config = Config {
        authorization: "fixture".into(),
        ..Config::default()
    };
    let body = json!({"key":"中文"});
    let url = server.url("/post");
    let direct: Value = personal_api_request(&config, &url, body.clone(), StorageType::PersonalNew)
        .await
        .unwrap();
    let via_client: Value = Client::new(config)
        .api_request_post(&url, body)
        .await
        .unwrap();
    assert_eq!(direct, via_client);
    request.assert_calls(2);
}
