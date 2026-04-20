use crate::client::ClientError;
use crate::client::endpoints::AUTH_TOKEN_REFRESH_URL;
use crate::config::Config;
use crate::{debug, warn};
use serde::Deserialize;

pub async fn login(
    token: &str,
    storage_type: &str,
    cloud_id: Option<&str>,
) -> Result<Config, ClientError> {
    debug!("正在解析 Token ...");

    let (account, token_info, expire_time) = parse_token(token)?;

    let config = Config {
        authorization: token.to_string(),
        account,
        storage_type: storage_type.to_string(),
        cloud_id: cloud_id.map(|s| s.to_string()),
        custom_upload_part_size: 0,
        report_real_size: true,
        use_large_thumbnail: false,
        personal_cloud_host: None,
        refresh_token: Some(token_info),
        token_expire_time: Some(expire_time),
        root_folder_id: None,
        user_domain_id: None,
    };

    Ok(config)
}

fn parse_token(token: &str) -> Result<(String, String, i64), ClientError> {
    let decoded = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, token)
        .map_err(|e| ClientError::Other(format!("Failed to decode token: {}", e)))?;

    let decode_str = String::from_utf8(decoded)
        .map_err(|e| ClientError::Other(format!("Invalid token encoding: {}", e)))?;

    let parts: Vec<&str> = decode_str.split(':').collect();
    if parts.len() < 3 {
        return Err(ClientError::Other(
            "Invalid token format: missing parts".to_string(),
        ));
    }

    let account = parts[1].to_string();
    let token_info = parts[2..].join(":");

    let token_parts: Vec<&str> = token_info.split('|').collect();
    if token_parts.len() < 4 {
        return Err(ClientError::Other(
            "Invalid token format: missing token info parts".to_string(),
        ));
    }

    let expire_time = token_parts[3]
        .parse::<i64>()
        .map_err(|_| ClientError::Other("Invalid expiration timestamp".to_string()))?;

    Ok((account, token_info, expire_time))
}

pub async fn refresh_token(config: &Config) -> Result<Config, ClientError> {
    debug!("Refreshing token for account: {}", config.account);

    if let Err(e) = check_token_expiration(config) {
        warn!("Token may be expired or invalid: {}", e);
        return Err(ClientError::TokenExpired);
    }

    let refresh_token = config
        .refresh_token
        .as_ref()
        .ok_or(ClientError::TokenExpired)?;

    let url = AUTH_TOKEN_REFRESH_URL;

    let body = format!(
        r#"<root><token>{}</token><account>{}</account><clienttype>656</clienttype></root>"#,
        refresh_token, config.account
    );

    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .build()?;

    let mut req_headers = reqwest::header::HeaderMap::new();
    req_headers.insert(
        "Content-Type",
        "application/xml;charset=UTF-8".parse().unwrap(),
    );
    req_headers.insert("Referer", "https://yun.139.com/".parse().unwrap());

    let resp = client
        .post(url)
        .headers(req_headers)
        .body(body)
        .send()
        .await?;

    let text = resp.text().await?;

    #[derive(Deserialize)]
    #[serde(rename = "root")]
    struct RefreshResp {
        #[serde(rename = "return")]
        return_code: String,
        #[allow(dead_code)]
        token: String,
        #[serde(rename = "accessToken")]
        access_token: String,
    }

    let refresh_resp: RefreshResp = serde_xml_rs::from_str(&text)
        .map_err(|e| ClientError::Other(format!("Failed to parse refresh response: {}", e)))?;

    if refresh_resp.return_code != "0" {
        return Err(ClientError::Other(format!(
            "Token refresh failed with code: {}",
            refresh_resp.return_code
        )));
    }

    let authorization = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        format!("pc:{}:{}", config.account, refresh_resp.access_token),
    );

    let mut new_config = config.clone();
    new_config.authorization = authorization;
    new_config.refresh_token = Some(refresh_resp.access_token);
    new_config.token_expire_time =
        Some(chrono::Utc::now().timestamp_millis() + 30 * 24 * 60 * 60 * 1000);

    Ok(new_config)
}

fn check_token_expiration(config: &Config) -> Result<(), ClientError> {
    let auth_parts: Vec<&str> = config.authorization.split(':').collect();
    if auth_parts.len() < 3 {
        return Err(ClientError::Other(
            "Invalid authorization format".to_string(),
        ));
    }

    let token_part = auth_parts[2];
    let token_parts: Vec<&str> = token_part.split('|').collect();

    if token_parts.len() < 4 {
        return Err(ClientError::Other("Invalid token format".to_string()));
    }

    let expiration = token_parts[3]
        .parse::<i64>()
        .map_err(|_| ClientError::Other("Invalid expiration timestamp".to_string()))?;

    let now = chrono::Utc::now().timestamp_millis();
    let remaining = expiration - now;

    if remaining < 0 {
        return Err(ClientError::TokenExpired);
    }

    if remaining > 15 * 24 * 60 * 60 * 1000 {
        return Ok(());
    }

    Ok(())
}

pub fn get_account(config: &Config) -> &str {
    &config.account
}
