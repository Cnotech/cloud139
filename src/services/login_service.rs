use super::list_service::{ListOptions, list};
use crate::client::api;
use crate::config::Config;
use crate::{info, success};

pub async fn login(token: &str) -> anyhow::Result<Config> {
    let mut config = crate::client::auth::parse_login_token(token)?;
    let path = Config::save_path();
    info!("正在校验 Token 可用性 (ls /) ...");
    config.personal_cloud_host = Some(
        api::HttpClientWrapper::new()
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
    config.save_to(&path)?;

    success!("Token 验证成功!");
    success!("配置文件已保存到: {}", path.display());

    Ok(config)
}
