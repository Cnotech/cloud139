use super::list::{ListOptions, list};
use crate::client::http::HttpClientWrapper;
use crate::config::Config;
use crate::info;

/// 验证 Token 并返回可用配置，供调用方选择保存位置。
pub async fn login(token: &str) -> anyhow::Result<Config> {
    let mut config = crate::client::auth::parse_login_token(token)?;
    info!("正在校验 Token 可用性 (ls /) ...");
    config.personal_cloud_host = Some(
        HttpClientWrapper::new()
            .fetch_personal_cloud_host(&config)
            .await?,
    );
    let options = ListOptions {
        path: "/",
        page: 1,
        page_size: 10,
    };
    list(&config, options)
        .await
        .map_err(|error| anyhow::anyhow!("登录校验失败: {error}"))?;
    Ok(config)
}
