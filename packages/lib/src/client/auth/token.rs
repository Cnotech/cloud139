use crate::config::Config;
use crate::debug;
use crate::error::ClientError;
use crate::protocol;

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
