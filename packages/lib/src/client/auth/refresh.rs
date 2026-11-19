use crate::client::http::insert_static_headers;
use crate::config::{Config, personal::PersonalConfig};
use crate::debug;
use crate::error::ClientError;
use crate::protocol;
use reqwest::{Client, header::HeaderMap};
use serde::Deserialize;
use std::time::Duration;

/// 续期端点及整个 HTTP 请求的超时。
#[derive(Debug, Clone, Copy)]
pub struct RefreshOptions<'a> {
    /// 自定义续期端点。默认使用官方端点。
    pub endpoint: Option<&'a str>,
    /// 单次请求的总超时，默认 10 秒。
    pub timeout: Duration,
}

impl Default for RefreshOptions<'_> {
    fn default() -> Self {
        Self {
            endpoint: None,
            timeout: Duration::from_secs(10),
        }
    }
}

/// 使用现有的续期凭据换取新 Token，并返回更新后的配置。
///
/// 原 Token 过期后仍可尝试续期。调用方可用 [`Config::save_to`] 持久化结果。
pub async fn refresh_token(
    config: &Config,
    options: RefreshOptions<'_>,
) -> Result<Config, ClientError> {
    refresh_personal_token(PersonalConfig::new(config)?, options).await
}

async fn refresh_personal_token(
    config: PersonalConfig<&Config>,
    options: RefreshOptions<'_>,
) -> Result<Config, ClientError> {
    debug!("开始续期 Token account={}", config.account);

    // 注意：不要在这里做过期判断。refresh token 在 authorization 过期后仍然有效，
    // 正是靠它来换取新 token；提前判死会让唯一能自愈的路径失效。
    let refresh_token = config
        .refresh_token
        .as_deref()
        .ok_or(ClientError::TokenExpired)?;

    let RefreshedToken {
        token: new_token,
        expire_seconds,
    } = request_refresh(&config.account, refresh_token, options).await?;

    let authorization = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        format!(
            "{}:{}:{}",
            protocol::PC_TOKEN_PREFIX,
            config.account,
            new_token
        ),
    );

    debug!("已续期 Token valid_days={}", expire_seconds / 86400);

    let mut new_config = (*config).clone();
    new_config.authorization = authorization;
    new_config.refresh_token = Some(new_token);
    new_config.token_expire_time =
        Some(chrono::Utc::now().timestamp_millis() + expire_seconds * 1000);

    Ok(new_config)
}

/// 达到 [`Config::is_token_refresh_due`] 的续期条件时发起请求并返回新配置，否则返回 `None`。
pub async fn refresh_token_if_due(
    config: &Config,
    options: RefreshOptions<'_>,
) -> Result<Option<Config>, ClientError> {
    let config = PersonalConfig::new(config)?;
    if !config.is_token_refresh_due() {
        return Ok(None);
    }
    refresh_personal_token(config, options).await.map(Some)
}

const DEFAULT_TOKEN_LIFETIME_SECS: i64 = 30 * 24 * 60 * 60;

struct RefreshedToken {
    token: String,
    expire_seconds: i64,
}

fn build_refresh_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    insert_static_headers(&mut headers, protocol::TOKEN_REFRESH_HEADERS);
    headers
}

fn refresh_http_client(timeout: Duration) -> Result<Client, ClientError> {
    // 总超时覆盖连接、响应头和完整响应体读取。
    Ok(Client::builder()
        .connect_timeout(Duration::from_secs(3).min(timeout))
        .timeout(timeout)
        .user_agent(protocol::TOKEN_REFRESH_USER_AGENT)
        .build()?)
}

fn build_refresh_body(account: &str, token: &str) -> String {
    format!(
        "<root><token>{}</token><account>{}</account><clienttype>{}</clienttype></root>",
        escape_text(token),
        escape_text(account),
        protocol::TOKEN_REFRESH_CLIENT_TYPE,
    )
}

fn escape_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

async fn request_refresh(
    account: &str,
    token: &str,
    options: RefreshOptions<'_>,
) -> Result<RefreshedToken, ClientError> {
    let url = options.endpoint.unwrap_or(protocol::AUTH_TOKEN_REFRESH_URL);
    let client = refresh_http_client(options.timeout)?;
    let text = client
        .post(url)
        .headers(build_refresh_headers())
        .body(build_refresh_body(account, token))
        .send()
        .await?
        .text()
        .await?;
    parse_refresh_response(&text)
}

fn parse_refresh_response(text: &str) -> Result<RefreshedToken, ClientError> {
    #[derive(Deserialize)]
    #[serde(rename = "root")]
    struct RefreshResp {
        #[serde(rename = "return")]
        return_code: String,
        #[serde(default)]
        desc: String,
        /// 新 token；接口实际把新 token 放在这里
        #[serde(default)]
        token: String,
        /// 新 token 有效期（秒），实测固定返回 2592000（30 天）
        #[serde(default, rename = "expiretime")]
        expire_time: Option<i64>,
        /// 部分场景为空，仅在 token 缺失时兜底使用
        #[serde(default, rename = "accessToken")]
        access_token: String,
    }

    let refresh_resp: RefreshResp = serde_xml_rs::from_str(text)
        .map_err(|error| ClientError::Other(format!("无法解析 Token 续期响应，{error}")))?;

    if refresh_resp.return_code != protocol::TOKEN_REFRESH_SUCCESS_CODE {
        return Err(ClientError::Other(format!(
            "续期 Token 失败 code={} desc={}",
            refresh_resp.return_code, refresh_resp.desc
        )));
    }

    // 以 <token> 为准；<accessToken> 实测为空，只在 token 缺失时兜底。
    let new_token = if refresh_resp.token.is_empty() {
        refresh_resp.access_token
    } else {
        refresh_resp.token
    };
    if new_token.is_empty() {
        return Err(ClientError::Other(
            "续期 Token 失败，响应未返回有效 Token".into(),
        ));
    }

    let expire_seconds = refresh_resp
        .expire_time
        .unwrap_or(DEFAULT_TOKEN_LIFETIME_SECS);
    Ok(RefreshedToken {
        token: new_token,
        expire_seconds,
    })
}
