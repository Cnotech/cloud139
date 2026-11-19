#![allow(dead_code)]

use cloud139::client::api::HttpClientWrapper;

#[test]
fn test_http_client_wrapper_creation() {
    let _wrapper = HttpClientWrapper::new();
}

#[test]
fn test_http_client_wrapper_default() {
    let _wrapper = HttpClientWrapper::default();
}

#[test]
fn test_http_client_wrapper_with_client() {
    let client = reqwest::Client::new();
    let _wrapper = HttpClientWrapper::with_client(client);
}
