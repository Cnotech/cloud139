use crate::config::Config;
use crate::debug;
use crate::error::ClientError;
use crate::protocol;
use std::time::Duration;

mod refresh;

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

pub fn parse_login_token(token: &str) -> Result<Config, ClientError> {
    let token = token
        .strip_prefix(protocol::BASIC_AUTH_PREFIX)
        .unwrap_or(token);
    debug!("开始解析 Token");
    let (account, refresh_token, expire_time) = parse_token(token)?;
    Ok(Config {
        authorization: token.into(),
        account,
        refresh_token: Some(refresh_token),
        token_expire_time: Some(expire_time),
        ..Config::default()
    })
}

fn parse_token(token: &str) -> Result<(String, String, i64), ClientError> {
    let decoded = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, token)
        .map_err(|_| ClientError::Other("Token 格式错误，无法解析".to_string()))?;

    let decode_str =
        String::from_utf8(decoded).map_err(|_| ClientError::Other("Token 编码无效".to_string()))?;

    let parts: Vec<&str> = decode_str.split(':').collect();
    if parts.len() < 3 {
        return Err(ClientError::Other(
            "Token 格式不完整，缺少必要字段".to_string(),
        ));
    }

    let account = parts[1].to_string();
    let token_info = parts[2..].join(":");

    let token_parts: Vec<&str> = token_info.split('|').collect();
    if token_parts.len() < 4 {
        return Err(ClientError::Other(
            "Token 信息不完整，可能已损坏".to_string(),
        ));
    }

    let expire_time = token_parts[3]
        .parse::<i64>()
        .map_err(|_| ClientError::Other("Token 过期时间无效".to_string()))?;

    Ok((account, token_info, expire_time))
}

/// 刷新 Token。默认使用官方端点，可通过 options 指定其他端点。
pub async fn refresh_token(
    config: &Config,
    options: RefreshOptions<'_>,
) -> Result<Config, ClientError> {
    config.require_personal()?;
    debug!("开始续期 Token account={}", config.account);

    // 注意：不要在这里做过期判断。refresh token 在 authorization 过期后仍然有效，
    // 正是靠它来换取新 token；提前判死会让唯一能自愈的路径失效。
    let refresh_token = config
        .refresh_token
        .as_deref()
        .ok_or(ClientError::TokenExpired)?;

    let refresh::RefreshedToken {
        token: new_token,
        expire_seconds,
    } = refresh::request(&config.account, refresh_token, options).await?;

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

    let mut new_config = config.clone();
    new_config.authorization = authorization;
    new_config.refresh_token = Some(new_token);
    new_config.token_expire_time =
        Some(chrono::Utc::now().timestamp_millis() + expire_seconds * 1000);

    Ok(new_config)
}

/// Token 达到续期条件时返回更新后的配置，否则返回 `None`。
pub async fn refresh_token_if_due(
    config: &Config,
    options: RefreshOptions<'_>,
) -> Result<Option<Config>, ClientError> {
    config.require_personal()?;
    if !config.is_token_refresh_due() {
        return Ok(None);
    }
    refresh_token(config, options).await.map(Some)
}
