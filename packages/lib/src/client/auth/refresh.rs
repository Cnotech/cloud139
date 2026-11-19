use super::RefreshOptions;
use crate::client::{ClientError, http::insert_static_headers};
use crate::protocol;
use crate::utils::xml::escape_text;
use reqwest::{Client, header::HeaderMap};
use serde::Deserialize;
use std::time::Duration;

const DEFAULT_TOKEN_LIFETIME_SECS: i64 = 30 * 24 * 60 * 60;

pub(super) struct RefreshedToken {
    pub(super) token: String,
    pub(super) expire_seconds: i64,
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

pub(super) async fn request(
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
