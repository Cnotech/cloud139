pub mod api;
pub mod auth;
pub mod error;

pub use crate::debug;
pub use crate::info;
pub use crate::utils::rand::{generate_rand_str, sort_json_value_to_string};
pub use api::headers::{
    CLIENT_INFO, DEVICE_INFO, MCLOUD_CHANNEL, MCLOUD_CHANNEL_SRC, MCLOUD_CLIENT, MCLOUD_VERSION,
    PC_CLIENT_INFO,
};
pub use api::{api_trait, endpoints, headers};
pub use endpoints::family;
pub use error::ClientError;
use serde::Deserialize;

pub struct Client {
    pub config: crate::config::Config,
    pub http_client: reqwest::Client,
}

impl Client {
    pub fn new(config: crate::config::Config) -> Self {
        Self {
            config,
            http_client: api::desktop_http_client(),
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
