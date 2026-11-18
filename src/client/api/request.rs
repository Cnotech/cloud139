use super::headers::{build_route_headers, build_signed_headers};
use crate::client::ClientError;
use crate::debug;
use crate::utils::generate_rand_str;
use reqwest::{Client, ClientBuilder, RequestBuilder};
use std::time::Duration;

const DESKTOP_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
const REFRESH_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36";

pub struct HttpClientWrapper {
    pub client: reqwest::Client,
}

impl Default for HttpClientWrapper {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpClientWrapper {
    pub fn new() -> Self {
        Self {
            client: http_client_builder()
                .build()
                .expect("HTTP 客户端初始化失败"),
        }
    }

    pub fn with_client(client: reqwest::Client) -> Self {
        Self { client }
    }
}

fn http_client_builder() -> ClientBuilder {
    Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .read_timeout(Duration::from_secs(30))
}

pub(in crate::client) fn desktop_http_client() -> Client {
    http_client_builder()
        .user_agent(DESKTOP_USER_AGENT)
        .build()
        .expect("HTTP 客户端初始化失败")
}

pub(super) fn refresh_http_client(timeout: Duration) -> Result<Client, ClientError> {
    // 总超时覆盖连接、响应头和完整响应体读取。
    Ok(Client::builder()
        .connect_timeout(Duration::from_secs(3).min(timeout))
        .timeout(timeout)
        .user_agent(REFRESH_USER_AGENT)
        .build()?)
}

pub(super) enum SignedEndpoint {
    Route,
    Personal,
}

fn build_signed_post(
    authorization: &str,
    url: &str,
    body: &serde_json::Value,
    client: &Client,
    endpoint: SignedEndpoint,
) -> Result<RequestBuilder, ClientError> {
    let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let nonce = generate_rand_str(16);
    let payload = body.to_string();
    let sign = crate::utils::crypto::calc_sign(&payload, &ts, &nonce);
    let mut headers = match endpoint {
        SignedEndpoint::Route => build_route_headers(authorization, &ts, &nonce, &sign),
        SignedEndpoint::Personal => build_signed_headers(authorization, &ts, &nonce, &sign, "1"),
    }?;
    headers.entry(reqwest::header::CONTENT_TYPE).or_insert(
        reqwest::header::HeaderValue::from_static("application/json"),
    );

    Ok(client.post(url).headers(headers).body(payload))
}

pub(super) async fn signed_post<T: for<'de> serde::Deserialize<'de>>(
    authorization: &str,
    url: &str,
    body: &serde_json::Value,
    client: &Client,
    endpoint: SignedEndpoint,
) -> Result<T, ClientError> {
    let request = build_signed_post(authorization, url, body, client, endpoint)?;
    debug!("发送签名请求 method=POST url={}", url);
    Ok(request.send().await?.error_for_status()?.json().await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_matches_the_serialized_request_body() {
        let body = serde_json::json!({ "name": "带引号的\"文件\"", "size": 42 });
        let client = HttpClientWrapper::new();
        for endpoint in [SignedEndpoint::Route, SignedEndpoint::Personal] {
            let request = build_signed_post(
                "fixture",
                "https://example.invalid/path",
                &body,
                &client.client,
                endpoint,
            )
            .unwrap()
            .build()
            .unwrap();

            let payload = request.body().unwrap().as_bytes().unwrap();
            let payload = std::str::from_utf8(payload).unwrap();
            let header = request.headers()["mcloud-sign"].to_str().unwrap();
            let (timestamp, signature) = header.split_once(',').unwrap();
            let (nonce, signature) = signature.split_once(',').unwrap();

            assert_eq!(
                serde_json::from_str::<serde_json::Value>(payload).unwrap(),
                body
            );
            assert_eq!(
                signature,
                crate::utils::crypto::calc_sign(payload, timestamp, nonce)
            );
        }
    }
}
