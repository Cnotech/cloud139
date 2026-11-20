use crate::debug;
use crate::error::ClientError;
use crate::protocol;
use crate::utils::crypto::{generate_rand_str, md5_hash};
use reqwest::{
    Client, ClientBuilder, RequestBuilder, StatusCode,
    header::{HeaderMap, HeaderValue},
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

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
        Self::with_client(
            http_client_builder()
                .read_timeout(Duration::from_secs(30))
                .build()
                .expect("HTTP 客户端初始化失败"),
        )
    }

    pub(crate) fn for_upload_parts() -> Self {
        // reqwest 的读取计时会覆盖请求体发送，分片上传单独按传输活动计时。
        Self::with_client(
            http_client_builder()
                .build()
                .expect("分片上传 HTTP 客户端初始化失败"),
        )
    }

    pub fn with_client(client: reqwest::Client) -> Self {
        Self { client }
    }

    pub(in crate::client) async fn signed_post<T: for<'de> serde::Deserialize<'de>>(
        &self,
        authorization: &str,
        url: &str,
        endpoint: &'static str,
        body: &serde_json::Value,
        build_headers: SignedHeaderBuilder,
    ) -> Result<T, ClientError> {
        let request = build_signed_post(authorization, url, body, &self.client, build_headers)?
            .build()
            .map_err(reqwest::Error::without_url)?;
        let trace = RequestTrace::start("POST", endpoint);
        let mut status = None;
        let result = async {
            let response = self.client.execute(request).await?;
            status = Some(response.status());
            response.error_for_status()?.json().await
        }
        .await;
        trace.finish(status, &result);
        Ok(result.map_err(reqwest::Error::without_url)?)
    }
}

pub(super) struct RequestTrace {
    method: &'static str,
    endpoint: &'static str,
    started: Option<(u64, Instant)>,
}

impl RequestTrace {
    /// 端点使用协议常量或固定标签，避免记录请求 URL。
    pub(super) fn start(method: &'static str, endpoint: &'static str) -> Self {
        static NEXT_REQUEST_ID: AtomicU64 = AtomicU64::new(1);
        let started = if log::log_enabled!(target: "cloud139::debug", log::Level::Debug) {
            let id = NEXT_REQUEST_ID.fetch_add(1, Ordering::Relaxed);
            debug!(
                "开始 API 请求 event=http_request_start request_id={id} method={method} endpoint={endpoint}"
            );
            Some((id, Instant::now()))
        } else {
            None
        };
        Self {
            method,
            endpoint,
            started,
        }
    }

    pub(super) fn finish<T>(self, status: Option<StatusCode>, result: &Result<T, reqwest::Error>) {
        let outcome = match result {
            Ok(_) => "ok",
            Err(error) if error.is_timeout() => "timeout",
            Err(error) if error.is_connect() => "connect_error",
            Err(error) if error.is_status() => "http_error",
            Err(error) if error.is_decode() => "decode_error",
            Err(_) => "transport_error",
        };
        self.record(status, outcome);
    }

    pub(super) fn record(self, status: Option<StatusCode>, outcome: &'static str) {
        if let Some((id, started)) = self.started {
            let status = status.map_or_else(|| "-".to_owned(), |code| code.as_u16().to_string());
            debug!(
                "结束 API 请求 event=http_request_end request_id={id} method={} endpoint={} status={status} elapsed_ms={:.3} outcome={outcome}",
                self.method,
                self.endpoint,
                started.elapsed().as_secs_f64() * 1000.0
            );
        }
    }
}

fn http_client_builder() -> ClientBuilder {
    Client::builder().connect_timeout(Duration::from_secs(10))
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
