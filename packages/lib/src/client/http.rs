use crate::debug;
use crate::error::ClientError;
use crate::protocol;
use crate::utils::crypto::{generate_rand_str, md5_hash};
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
    let sign = calc_sign(&payload, &ts, &nonce);
    let mut headers = build_headers(authorization, &ts, &nonce, &sign)?;
    headers
        .entry(reqwest::header::CONTENT_TYPE)
        .or_insert(HeaderValue::from_static(protocol::JSON_CONTENT_TYPE));

    Ok(client.post(url).headers(headers).body(payload))
}

pub fn encode_uri_component(s: &str) -> String {
    let mut result = String::new();
    for c in s.chars() {
        match c {
            // 编码行为与 JavaScript 的 `encodeURIComponent` 及 AList/OpenList 签名逻辑一致。
            '!' | '\'' | '(' | ')' | '*' => result.push(c),
            ' ' => result.push_str("%20"),
            _ => {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '~' {
                    result.push(c);
                } else {
                    for b in c.to_string().as_bytes() {
                        result.push_str(&format!("%{:02X}", b));
                    }
                }
            }
        }
    }
    result
}

pub fn calc_sign(body: &str, ts: &str, rand_str: &str) -> String {
    let encoded = encode_uri_component(body);
    let mut chars: Vec<char> = encoded.chars().collect();
    chars.sort();
    let sorted: String = chars.into_iter().collect();

    let body_base64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &sorted);

    let hash1 = md5_hash(&body_base64);
    let hash2 = md5_hash(&format!("{}:{}", ts, rand_str));

    let combined = format!("{}{}", hash1, hash2);
    md5_hash(&combined).to_uppercase()
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
            assert_eq!(signature, calc_sign(payload, timestamp, nonce));
        }
    }
}
