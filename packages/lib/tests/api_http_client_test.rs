#![allow(dead_code)]

use cloud139::client::http::HttpClientWrapper;
use cloud139::config::Config;

#[tokio::test]
async fn test_get_personal_cloud_host_with_cached_host() {
    let mut config = Config::default();
    config.personal_cloud_host = Some("https://cached.example.com".to_string());

    let wrapper = HttpClientWrapper::new();
    let result = wrapper.get_personal_cloud_host(&mut config).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "https://cached.example.com".to_string());
}

#[tokio::test]
async fn test_get_personal_cloud_host_with_empty_account() {
    let mut config = Config::default();
    config.account = "".to_string();
    config.authorization = "test".to_string();

    let wrapper = HttpClientWrapper::new();
    let result = wrapper.get_personal_cloud_host(&mut config).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn personal_post_sends_expected_signed_headers() {
    use cloud139::client::api::headers::PC_CLIENT_INFO;
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
    let response: Value = HttpClientWrapper::new()
        .personal_api_request(&config, &url, &body)
        .await
        .unwrap();
    assert_eq!(response, json!({"success": true}));
    request.assert_calls(1);
}
