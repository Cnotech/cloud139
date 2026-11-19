use super::list::{ListOptions, list_personal};
use crate::client::http::HttpClientWrapper;
use crate::config::{Config, personal::PersonalConfig};
use crate::info;

/// 验证 Token 并返回可用配置，供调用方选择保存位置。
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
