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
    let list_args = crate::commands::list::ListArgs {
        recursive: false,
        path: "/".to_string(),
        page: 1,
        page_size: 10,
        output: None,
    };
    crate::services::list(&config, &list_args)
        .await
        .map_err(|error| anyhow::anyhow!("登录校验失败: {error}"))?;
    config.save_to(&path)?;

    success!("Token 验证成功!");
    success!("配置文件已保存到: {}", path.display());

    Ok(config)
}
