pub mod api;
pub mod api_trait;
pub mod auth;
pub mod endpoints;
pub mod error;
pub mod headers;
pub mod storage_type;

pub use endpoints::family;
pub use error::ClientError;
pub use storage_type::StorageType;
pub use crate::info;
use serde::Deserialize;
use crate::client::endpoints::group;
pub use crate::utils::rand::{generate_rand_str, sort_json_value_to_string};

const KEY_HEX_1: &str = "73634235495062495331515373756c734e7253306c673d3d";

pub const MCLOUD_VERSION: &str = "7.14.0";
pub const MCLOUD_CLIENT: &str = "10701";
pub const MCLOUD_CHANNEL: &str = "1000101";
pub const MCLOUD_CHANNEL_SRC: &str = "10000034";
pub const DEVICE_INFO: &str = "||9|7.14.0|chrome|120.0.0.0|||windows 10||zh-CN|||";
pub const CLIENT_INFO: &str = "||9|7.14.0|chrome|120.0.0.0|||windows 10||zh-CN|||dW5kZWZpbmVk||";

pub struct Client {
    pub config: crate::config::Config,
    pub http_client: reqwest::Client,
}

impl Client {
    pub fn new(config: crate::config::Config) -> Self {
        let http_client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .build()
            .expect("Failed to build HTTP client");

        Self {
            config,
            http_client,
        }
    }

    pub async fn login(
        token: String,
        storage_type: String,
        cloud_id: Option<String>,
    ) -> Result<Self, ClientError> {
        let config = auth::login(&token, &storage_type, cloud_id.as_deref()).await?;
        config.save()?;
        Ok(Self::new(config))
    }

    pub async fn refresh_token_if_needed(&mut self) -> Result<(), ClientError> {
        if self.config.is_token_expired() {
            info!("Token expired, refreshing...");
            let new_config = auth::refresh_token(&self.config).await?;
            new_config.save()?;
            self.config = new_config;
        }
        Ok(())
    }

    pub async fn api_request_post<T: for<'de> Deserialize<'de>>(
        &self,
        url: &str,
        body: serde_json::Value,
    ) -> Result<T, ClientError> {
        let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let rand_str = generate_rand_str(16);
        let body_str = body.to_string();
        let sign = crate::utils::crypto::calc_sign(&body_str, &ts, &rand_str);

        let headers = self.build_headers(&ts, &rand_str, &sign)?;

        let resp = self
            .http_client
            .post(url)
            .headers(headers)
            .json(&body)
            .send()
            .await?;

        let result: T = resp.json().await?;
        Ok(result)
    }

    fn build_headers(
        &self,
        ts: &str,
        rand_str: &str,
        sign: &str,
    ) -> Result<reqwest::header::HeaderMap, ClientError> {
        headers::build_signed_headers(
            &self.config.authorization,
            ts,
            rand_str,
            sign,
            self.config.storage_type().svc_type(),
        )
    }

    pub async fn and_album_request<T: for<'de> Deserialize<'de>>(
        &self,
        pathname: &str,
        body: serde_json::Value,
    ) -> Result<T, ClientError> {
        let url = format!(
            "{}{}",
            family::ALBUM_BASE_URL, pathname
        );

        let headers = self.build_and_album_headers()?;

        let key1 = hex::decode(KEY_HEX_1).map_err(|e| ClientError::Other(e.to_string()))?;

        let sorted_body_str = sort_json_value_to_string(&body);

        let iv = vec![0u8; 16];
        let encrypted =
            crate::utils::crypto::aes_cbc_encrypt(sorted_body_str.as_bytes(), &key1, &iv)
                .map_err(|e| ClientError::Other(e.to_string()))?;

        let mut payload = iv.clone();
        payload.extend(encrypted);

        let payload_base64 =
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &payload);

        let resp = self
            .http_client
            .post(&url)
            .headers(headers)
            .body(payload_base64)
            .send()
            .await?;

        let resp_body = resp.bytes().await?;
        let resp_str = String::from_utf8_lossy(&resp_body);

        let decrypted = if resp_str.trim_start().starts_with('{') {
            resp_body.to_vec()
        } else {
            crate::utils::crypto::aes_cbc_decrypt(&resp_body, &key1, &iv)
                .map_err(|e| ClientError::Other(e.to_string()))?
        };

        let result: T = serde_json::from_slice(&decrypted)
            .map_err(|e| ClientError::Other(format!("Failed to parse response: {}", e)))?;

        Ok(result)
    }

    pub async fn isbo_post<T: for<'de> Deserialize<'de>>(
        &self,
        pathname: &str,
        body: serde_json::Value,
    ) -> Result<T, ClientError> {
        let url = format!("{}{}", group::MUTUAL_BASE_URL, pathname);

        let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let rand_str = generate_rand_str(16);
        let body_str = body.to_string();
        let sign = crate::utils::crypto::calc_sign(&body_str, &ts, &rand_str);

        let headers = headers::build_group_signed_headers(
            &self.config.authorization,
            &ts,
            &rand_str,
            &sign,
            "2",
        )?;

        let resp = self
            .http_client
            .post(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await?;

        let result: T = resp.json().await?;
        Ok(result)
    }

    fn build_and_album_headers(&self) -> Result<reqwest::header::HeaderMap, ClientError> {
        use reqwest::header::HeaderValue;

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("Host", HeaderValue::from_static("group.yun.139.com"));
        let auth_value = format!("Basic {}", self.config.authorization);
        let auth_header: reqwest::header::HeaderValue = auth_value
            .parse()
            .map_err(|e: reqwest::header::InvalidHeaderValue| ClientError::InvalidHeader(e.to_string()))?;
        headers.insert("authorization", auth_header);
        headers.insert("x-svctype", HeaderValue::from_static("2"));
        headers.insert("hcy-cool-flag", HeaderValue::from_static("1"));
        headers.insert("api-version", HeaderValue::from_static("v2"));
        headers.insert("x-huawei-channelsrc", HeaderValue::from_static("10246600"));
        headers.insert("x-sdk-channelsrc", HeaderValue::from_static(""));
        headers.insert("x-mm-source", HeaderValue::from_static("0"));
        headers.insert("x-deviceinfo", HeaderValue::from_static("1|127.0.0.1|1|12.3.2|Xiaomi|23116PN5BC||02-00-00-00-00-00|android 15|1440x3200|android|zh||||032|0|"));
        headers.insert(
            "content-type",
            HeaderValue::from_static("application/json; charset=utf-8"),
        );
        headers.insert("user-agent", HeaderValue::from_static("okhttp/4.11.0"));
        headers.insert("accept-encoding", HeaderValue::from_static("gzip"));
        Ok(headers)
    }
}