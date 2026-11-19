//! 验证登录 Token 并生成个人云配置。

use super::list::{ListOptions, list_personal};
use crate::client::http::HttpClientWrapper;
use crate::config::{Config, personal::PersonalConfig};
use crate::info;

/// 解析登录 Token，通过读取云端根目录验证凭据，再返回配置。
///
/// 调用方可用 [`Config::save_to`] 将配置保存到指定路径。
pub async fn login(token: &str) -> anyhow::Result<Config> {
    let mut config = PersonalConfig::new(crate::client::auth::token::parse_login_token(token)?)?;
    info!("正在校验 Token 可用性 (ls /) ...");
    let host = HttpClientWrapper::new()
        .resolve_personal_cloud_host(config.as_ref())
        .await?;
    config.set_cloud_host(host);
    let options = ListOptions {
        path: "/",
        page: 1,
        page_size: 10,
    };
    list_personal(config.as_ref(), options)
        .await
        .map_err(|error| anyhow::anyhow!("登录校验失败: {error}"))?;
    Ok(config.into_inner())
}
