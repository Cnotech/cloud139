use crate::client::ClientError;
use crate::client::api::refresh;
pub use crate::client::api::refresh::RefreshOptions;
use crate::config::{Config, ConfigUpdate, config_load_candidates};
use crate::{debug, warn};
use std::path::PathBuf;

pub async fn login(
    token: &str,
    storage_type: &str,
    cloud_id: Option<&str>,
) -> Result<Config, ClientError> {
    Config::validate_storage_type(storage_type)?;
    debug!("开始解析 Token");

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
        format!("pc:{}:{}", config.account, new_token),
    );

    debug!("已续期 Token valid_days={}", expire_seconds / 86400);

    let mut new_config = config.clone();
    new_config.authorization = authorization;
    new_config.refresh_token = Some(new_token);
    new_config.token_expire_time =
        Some(chrono::Utc::now().timestamp_millis() + expire_seconds * 1000);

    Ok(new_config)
}

/// 按需续期：需要续期时返回新配置，不需要时返回 None。
pub async fn refresh_if_needed(
    config: &Config,
    options: RefreshOptions<'_>,
) -> Result<Option<Config>, ClientError> {
    config.require_personal()?;
    if !config.needs_refresh() {
        return Ok(None);
    }
    refresh_token(config, options).await.map(Some)
}

/// 命令启动时按需续期并落盘。
///
/// 失败只告警、不阻断：此刻的 authorization 可能仍然可用，
/// 而续期失败最常见的原因是网络；真正过期后由命令自己报 401。
pub async fn maybe_refresh_config() -> Result<(), ClientError> {
    refresh_config_if_needed(&config_load_candidates(), RefreshOptions::default()).await
}

async fn refresh_config_if_needed(
    candidates: &[PathBuf],
    options: RefreshOptions<'_>,
) -> Result<(), ClientError> {
    let (config, path) = match Config::load_from_candidates(candidates) {
        Ok(loaded) => loaded,
        // 没有配置时交给命令自己报「配置文件未找到」，这里不抢错误。
        Err(_) => return Ok(()),
    };
    config.require_personal()?;
    if !config.needs_refresh() {
        return Ok(());
    }

    // 路径已经固定。锁忙时跳过，不让另一个进程的续期阻塞普通命令。
    let update = match ConfigUpdate::load(&path) {
        Ok(update) => update,
        Err(err) => {
            warn!("自动续期跳过（继续使用现有 Token）: {}", err);
            return Ok(());
        }
    };
    // 等待锁之前可能已有进程完成了续期，必须使用锁内重新读取的配置判断。
    match refresh_if_needed(&update.config, options).await {
        Ok(Some(new_config)) => {
            update
                .save(&new_config)
                .map_err(|err| ClientError::Other(format!("保存配置失败: {}", err)))?;
            debug!("已将续期后的 Token 写回配置 path={}", path.display());
        }
        Ok(None) => {}
        Err(ClientError::UnsupportedStorage) => return Err(ClientError::UnsupportedStorage),
        Err(err) => {
            warn!("自动续期失败（继续使用现有 Token）: {}", err);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests;

pub fn get_account(config: &Config) -> &str {
    &config.account
}
