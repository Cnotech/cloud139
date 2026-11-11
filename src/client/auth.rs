use crate::client::ClientError;
use crate::client::endpoints::AUTH_TOKEN_REFRESH_URL;
use crate::config::{Config, ConfigUpdate, config_load_candidates};
use crate::{debug, warn};
use serde::Deserialize;
use std::path::PathBuf;
use std::time::Duration;

const REFRESH_CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const REFRESH_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

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

pub async fn refresh_token(config: &Config) -> Result<Config, ClientError> {
    refresh_token_at(config, AUTH_TOKEN_REFRESH_URL).await
}

/// 与 refresh_token 相同，但可以指定接口地址（测试用）。
pub async fn refresh_token_at(config: &Config, url: &str) -> Result<Config, ClientError> {
    refresh_token_with_timeout(config, url, REFRESH_REQUEST_TIMEOUT).await
}

async fn refresh_token_with_timeout(
    config: &Config,
    url: &str,
    timeout: Duration,
) -> Result<Config, ClientError> {
    debug!("Refreshing token for account: {}", config.account);

    // 注意：不要在这里做过期判断。refresh token 在 authorization 过期后仍然有效，
    // 正是靠它来换取新 token；提前判死会让唯一能自愈的路径失效。
    let refresh_token = config
        .refresh_token
        .as_ref()
        .ok_or(ClientError::TokenExpired)?;

    let body = format!(
        r#"<root><token>{}</token><account>{}</account><clienttype>656</clienttype></root>"#,
        refresh_token, config.account
    );

    let client = reqwest::Client::builder()
        .connect_timeout(REFRESH_CONNECT_TIMEOUT.min(timeout))
        // reqwest 的总超时包含连接、响应头和完整响应体读取。
        .timeout(timeout)
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

    let refresh_resp: RefreshResp = serde_xml_rs::from_str(&text)
        .map_err(|e| ClientError::Other(format!("Failed to parse refresh response: {}", e)))?;

    if refresh_resp.return_code != "0" {
        return Err(ClientError::Other(format!(
            "Token refresh failed: code={}, desc={}",
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
            "Token refresh failed: empty token in response".to_string(),
        ));
    }

    let expire_seconds = refresh_resp
        .expire_time
        .unwrap_or(DEFAULT_TOKEN_LIFETIME_SECS);
    let authorization = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        format!("pc:{}:{}", config.account, new_token),
    );

    debug!("Token 刷新成功，新有效期 {} 天", expire_seconds / 86400);

    let mut new_config = config.clone();
    new_config.authorization = authorization;
    new_config.refresh_token = Some(new_token);
    new_config.token_expire_time =
        Some(chrono::Utc::now().timestamp_millis() + expire_seconds * 1000);

    Ok(new_config)
}

/// 接口没返回 expiretime 时的兜底有效期（实测固定 30 天）。
const DEFAULT_TOKEN_LIFETIME_SECS: i64 = 30 * 24 * 60 * 60;

/// 按需续期：需要续期时返回新配置，不需要时返回 None。
pub async fn refresh_if_needed(config: &Config) -> Result<Option<Config>, ClientError> {
    refresh_if_needed_at(config, AUTH_TOKEN_REFRESH_URL).await
}

/// 与 refresh_if_needed 相同，但可以指定接口地址（测试用）。
pub async fn refresh_if_needed_at(
    config: &Config,
    url: &str,
) -> Result<Option<Config>, ClientError> {
    if !config.needs_refresh() {
        return Ok(None);
    }
    refresh_token_at(config, url).await.map(Some)
}

/// 命令启动时按需续期并落盘。
///
/// 失败只告警、不阻断：此刻的 authorization 可能仍然可用，
/// 而续期失败最常见的原因是网络；真正过期后由命令自己报 401。
pub async fn maybe_refresh_config() -> Result<(), ClientError> {
    maybe_refresh_config_at(
        &config_load_candidates(),
        AUTH_TOKEN_REFRESH_URL,
        REFRESH_REQUEST_TIMEOUT,
    )
    .await
}

async fn maybe_refresh_config_at(
    candidates: &[PathBuf],
    url: &str,
    timeout: Duration,
) -> Result<(), ClientError> {
    let (config, path) = match Config::load_from_candidates(candidates) {
        Ok(loaded) => loaded,
        // 没有配置时交给命令自己报「配置文件未找到」，这里不抢错误。
        Err(_) => return Ok(()),
    };
    if !config.needs_refresh() {
        return Ok(());
    }

    // 路径已经固定。锁忙时跳过，不让另一个进程的续期阻塞普通命令。
    let update = match ConfigUpdate::load(&path) {
        Ok(update) => update,
        Err(err) => {
            warn!("自动续期跳过（继续使用现有 token）: {}", err);
            return Ok(());
        }
    };
    // 等待锁之前可能已有进程完成了续期，必须使用锁内重新读取的配置判断。
    if !update.config.needs_refresh() {
        return Ok(());
    }
    match refresh_token_with_timeout(&update.config, url, timeout).await {
        Ok(new_config) => {
            update
                .save(&new_config)
                .map_err(|err| ClientError::Other(format!("保存配置失败: {}", err)))?;
            debug!("Token 已自动续期并写回配置: {}", path.display());
        }
        Err(err) => {
            warn!("自动续期失败（继续使用现有 token）: {}", err);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests;

pub fn get_account(config: &Config) -> &str {
    &config.account
}
