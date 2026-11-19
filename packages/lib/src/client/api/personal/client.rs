use crate::client::{
    api::{headers::build_personal_headers, route::query_route_policy},
    http::HttpClientWrapper,
};
use crate::config::{Config, personal::PersonalConfig};
use crate::debug;
use crate::error::ClientError;
use crate::protocol;
use std::{borrow::Cow, time::Duration};

#[derive(Clone, Copy)]
pub(crate) enum ListingRetry {
    None,
    Transient {
        retries: u32,
        backoff_secs: u64,
        timeout: Option<Duration>,
    },
}

impl ListingRetry {
    pub(crate) const DIRECTORY: Self = Self::Transient {
        retries: 2,
        backoff_secs: 1,
        timeout: Some(Duration::from_secs(20)),
    };
}

pub(crate) struct PersonalClient<'a> {
    pub(in crate::client::api) config: PersonalConfig<&'a Config>,
    host: Cow<'a, str>,
    pub(super) http_client: &'a HttpClientWrapper,
    pub(super) retry: ListingRetry,
}

impl<'a> PersonalClient<'a> {
    pub(crate) async fn connect(
        config: PersonalConfig<&'a Config>,
        client: &'a HttpClientWrapper,
        retry: ListingRetry,
    ) -> Result<Self, ClientError> {
        let host = client.resolve_personal_cloud_host(config).await?;
        Ok(Self::with_host(config, host, client, retry))
    }

    pub(crate) fn with_host(
        config: PersonalConfig<&'a Config>,
        host: impl Into<Cow<'a, str>>,
        http_client: &'a HttpClientWrapper,
        retry: ListingRetry,
    ) -> Self {
        Self {
            config,
            host: host.into(),
            http_client,
            retry,
        }
    }

    pub(crate) fn with_listing_retry(&self, retry: ListingRetry) -> PersonalClient<'_> {
        PersonalClient::with_host(self.config, self.host.as_ref(), self.http_client, retry)
    }

    pub(crate) fn host(&self) -> &str {
        &self.host
    }

    pub(in crate::client::api) async fn request<T: for<'de> serde::Deserialize<'de>>(
        &self,
        endpoint: &str,
        body: &serde_json::Value,
    ) -> Result<T, ClientError> {
        self.http_client
            .personal_request(self.config, &format!("{}{endpoint}", self.host), body)
            .await
    }
}

impl HttpClientWrapper {
    pub async fn get_personal_cloud_host(
        &self,
        config: &mut Config,
    ) -> Result<String, ClientError> {
        let host = self
            .resolve_personal_cloud_host(PersonalConfig::new(&*config)?)
            .await?;
        config
            .personal_cloud_host
            .get_or_insert_with(|| host.clone());
        Ok(host)
    }

    pub(crate) async fn resolve_personal_cloud_host(
        &self,
        config: PersonalConfig<&Config>,
    ) -> Result<String, ClientError> {
        if let Some(ref host) = config.personal_cloud_host {
            debug!("使用缓存的个人云地址 host={}", host);
            return Ok(host.clone());
        }

        let host = query_route_policy(
            self,
            &config.account,
            protocol::PERSONAL_ACCOUNT_TYPE,
            &config.authorization,
        )
        .await?
        .into_iter()
        .find(|p| p.mod_name.as_deref() == Some(protocol::PERSONAL_MODULE))
        .map(|p| p.https_url.unwrap_or_default())
        .ok_or_else(|| ClientError::Other("未找到个人云地址".into()))?;

        debug!("已解析个人云地址 host={}", host);
        Ok(host)
    }

    pub async fn personal_api_request<T: for<'de> serde::Deserialize<'de>>(
        &self,
        config: &Config,
        url: &str,
        body: &serde_json::Value,
    ) -> Result<T, ClientError> {
        self.personal_request(PersonalConfig::new(config)?, url, body)
            .await
    }

    async fn personal_request<T: for<'de> serde::Deserialize<'de>>(
        &self,
        config: PersonalConfig<&Config>,
        url: &str,
        body: &serde_json::Value,
    ) -> Result<T, ClientError> {
        self.signed_post(&config.authorization, url, body, build_personal_headers)
            .await
    }
}
