use crate::client::ClientError;
use crate::debug;
use crate::protocol;
use crate::utils::rand::generate_rand_str;
use reqwest::{
    Client, ClientBuilder, RequestBuilder,
    header::{HeaderMap, HeaderValue},
};
use std::time::Duration;

type SignedHeaderBuilder = fn(&str, &str, &str, &str) -> Result<HeaderMap, ClientError>;

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

    pub(in crate::client) async fn signed_post<T: for<'de> serde::Deserialize<'de>>(
        &self,
        authorization: &str,
        url: &str,
        body: &serde_json::Value,
        build_headers: SignedHeaderBuilder,
    ) -> Result<T, ClientError> {
        let request = build_signed_post(authorization, url, body, &self.client, build_headers)?;
        debug!("发送签名请求 method=POST url={}", url);
        Ok(request.send().await?.error_for_status()?.json().await?)
    }
}

fn http_client_builder() -> ClientBuilder {
    Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .read_timeout(Duration::from_secs(30))
}

fn build_signed_post(
    authorization: &str,
    url: &str,
    body: &serde_json::Value,
    client: &Client,
    build_headers: SignedHeaderBuilder,
) -> Result<RequestBuilder, ClientError> {
    let ts = chrono::Local::now()
        .format(protocol::SIGNATURE_TIME_FORMAT)
        .to_string();
    let nonce = generate_rand_str(protocol::SIGNATURE_NONCE_LENGTH);
    let payload = body.to_string();
    let sign = crate::utils::crypto::calc_sign(&payload, &ts, &nonce);
    let mut headers = build_headers(authorization, &ts, &nonce, &sign)?;
    headers
        .entry(reqwest::header::CONTENT_TYPE)
        .or_insert(HeaderValue::from_static(protocol::JSON_CONTENT_TYPE));

    Ok(client.post(url).headers(headers).body(payload))
}

pub(super) fn insert_static_headers(
    headers: &mut HeaderMap,
    values: &[(&'static str, &'static str)],
) {
    for &(name, value) in values {
        headers.insert(name, HeaderValue::from_static(value));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::api::headers::{build_personal_headers, build_route_headers};

    #[test]
    fn signature_matches_the_serialized_request_body() {
        let body = serde_json::json!({ "name": "带引号的\"文件\"", "size": 42 });
        let client = HttpClientWrapper::new();
        let builders: [(SignedHeaderBuilder, &str); 2] = [
            (build_route_headers, "application/json;charset=UTF-8"),
            (build_personal_headers, "application/json"),
        ];
        for (build_headers, content_type) in builders {
            let request = build_signed_post(
                "fixture",
                "https://example.invalid/path",
                &body,
                &client.client,
                build_headers,
            )
            .unwrap()
            .build()
            .unwrap();

            assert_eq!(request.headers()["content-type"], content_type);
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
