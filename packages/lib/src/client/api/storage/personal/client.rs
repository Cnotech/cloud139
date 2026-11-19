use crate::client::{
    ClientError,
    api::common::query_route_policy,
    http::{HttpClientWrapper, build_signed_headers},
};
use crate::config::Config;
use crate::debug;
use crate::protocol;
use reqwest::header::HeaderMap;
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
    pub(super) config: &'a Config,
    host: Cow<'a, str>,
    pub(super) http_client: &'a HttpClientWrapper,
    pub(super) retry: ListingRetry,
}

impl<'a> PersonalClient<'a> {
    pub(crate) async fn connect(
        config: &'a Config,
        client: &'a HttpClientWrapper,
        retry: ListingRetry,
    ) -> Result<Self, ClientError> {
        let host = client.get_personal_cloud_host(&mut config.clone()).await?;
        Ok(Self::with_host(config, host, client, retry))
    }

    pub(crate) fn with_host(
        config: &'a Config,
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

    pub(super) async fn request<T: for<'de> serde::Deserialize<'de>>(
        &self,
        endpoint: &str,
        body: &serde_json::Value,
    ) -> Result<T, ClientError> {
        self.http_client
            .personal_api_request(self.config, &format!("{}{endpoint}", self.host), body)
            .await
    }
}

impl HttpClientWrapper {
    pub async fn get_personal_cloud_host(
        &self,
        config: &mut Config,
    ) -> Result<String, ClientError> {
        config.require_personal()?;
        if let Some(ref host) = config.personal_cloud_host {
            debug!("使用缓存的个人云地址 host={}", host);
            return Ok(host.clone());
        }

        let host = self.fetch_personal_cloud_host(config).await?;
        config.personal_cloud_host = Some(host.clone());
        Ok(host)
    }

    pub(crate) async fn fetch_personal_cloud_host(
        &self,
        config: &Config,
    ) -> Result<String, ClientError> {
        config.require_personal()?;
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
        config.require_personal()?;
        self.signed_post(&config.authorization, url, body, build_personal_headers)
            .await
    }
}

pub fn build_personal_headers(
    authorization: &str,
    ts: &str,
    rand_str: &str,
    sign: &str,
) -> Result<HeaderMap, ClientError> {
    build_signed_headers(
        protocol::PERSONAL_HEADERS,
        authorization,
        ts,
        rand_str,
        sign,
    )
}
