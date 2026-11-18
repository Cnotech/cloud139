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
        let http_client = api::http_client_builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .build()
            .expect("HTTP 客户端初始化失败");

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
        api::personal_post(
            &self.config,
            url,
            &body,
            crate::domain::StorageType::PersonalNew,
            &self.http_client,
        )
        .await
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
