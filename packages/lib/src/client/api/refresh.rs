use super::{
    endpoints::AUTH_TOKEN_REFRESH_URL, headers::build_refresh_headers, request::refresh_http_client,
};
use crate::client::ClientError;
use serde::Deserialize;
use std::time::Duration;

const DEFAULT_TOKEN_LIFETIME_SECS: i64 = 30 * 24 * 60 * 60;

/// 续期端点及整个 HTTP 请求的超时。
#[derive(Debug, Clone, Copy)]
pub struct RefreshOptions<'a> {
    pub endpoint: Option<&'a str>,
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

pub(in crate::client) struct RefreshedToken {
    pub token: String,
    pub expire_seconds: i64,
}

fn build_refresh_body(account: &str, token: &str) -> String {
    fn escape_text(value: &str) -> String {
        value
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    }

    format!(
        "<root><token>{}</token><account>{}</account><clienttype>656</clienttype></root>",
        escape_text(token),
        escape_text(account)
    )
}

pub(in crate::client) async fn request(
    account: &str,
    token: &str,
    options: RefreshOptions<'_>,
) -> Result<RefreshedToken, ClientError> {
    let url = options.endpoint.unwrap_or(AUTH_TOKEN_REFRESH_URL);
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

    if refresh_resp.return_code != "0" {
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
