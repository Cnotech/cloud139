pub mod api;
pub mod api_trait;
pub mod auth;
pub mod endpoints;
pub mod error;
pub mod headers;

pub use crate::debug;
pub use crate::info;
pub use crate::utils::rand::{generate_rand_str, sort_json_value_to_string};
pub use endpoints::family;
pub use error::ClientError;
use serde::Deserialize;

pub const MCLOUD_VERSION: &str = "7.14.0";
pub const MCLOUD_CLIENT: &str = "10701";
pub const MCLOUD_CHANNEL: &str = "1000101";
pub const MCLOUD_CHANNEL_SRC: &str = "10000034";
pub const DEVICE_INFO: &str = "||9|7.14.0|chrome|120.0.0.0|||windows 10||zh-CN|||";
pub const CLIENT_INFO: &str = "||9|7.14.0|chrome|120.0.0.0|||windows 10||zh-CN|||dW5kZWZpbmVk||";

// Personal-cloud file APIs use the desktop client identity; authentication and
// legacy family/group protocols retain their existing fingerprints.
pub const PC_CLIENT_INFO: &str =
    "||13|7.14.0|chrome|120.0.0.0|||windows 10||zh-CN|||dW5kZWZpbmVk||";

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
        // 注意：config.save() 不在此处调用。
        // 持久化配置是调用方（commands/login.rs）的职责，避免测试副作用污染真实配置文件。
        Ok(Self::new(config))
    }

    pub async fn api_request_post<T: for<'de> Deserialize<'de>>(
        &self,
        url: &str,
        body: serde_json::Value,
    ) -> Result<T, ClientError> {
        self.config.require_personal()?;
        let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let rand_str = generate_rand_str(16);
        let body_str = body.to_string();
        let sign = crate::utils::crypto::calc_sign(&body_str, &ts, &rand_str);

        let headers = self.build_headers(&ts, &rand_str, &sign)?;

        debug!("api_request_post: POST {}", url);
        let resp = self
            .http_client
            .post(url)
            .headers(headers)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;

        let result: T = resp.json().await?;
        Ok(result)
    }

    fn build_headers(
        &self,
        ts: &str,
        rand_str: &str,
        sign: &str,
    ) -> Result<reqwest::header::HeaderMap, ClientError> {
        headers::build_signed_headers(&self.config.authorization, ts, rand_str, sign, "1")
    }

    pub async fn and_album_request<T: for<'de> Deserialize<'de>>(
        &self,
        _pathname: &str,
        _body: serde_json::Value,
    ) -> Result<T, ClientError> {
        Err(ClientError::UnsupportedStorage)
    }

    pub async fn isbo_post<T: for<'de> Deserialize<'de>>(
        &self,
        _pathname: &str,
        _body: serde_json::Value,
    ) -> Result<T, ClientError> {
        Err(ClientError::UnsupportedStorage)
    }
}
