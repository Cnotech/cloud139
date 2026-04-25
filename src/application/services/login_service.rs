use crate::{debug, info, success, warn};

pub async fn login(
    token: &str,
    storage_type: &str,
    cloud_id: Option<&str>,
) -> anyhow::Result<crate::config::Config> {
    let token = token
        .strip_prefix("Basic ")
        .map(|s| s.to_string())
        .unwrap_or_else(|| token.to_string());

    let config = crate::client::auth::login(&token, storage_type, cloud_id).await?;
    config.save()?;

    info!("正在校验 Token 可用性 (ls /) ...");
    let list_args = crate::commands::list::ListArgs {
        path: "/".to_string(),
        page: 1,
        page_size: 10,
        output: None,
    };
    if let Err(e) = crate::application::services::list(&config, &list_args).await {
        warn!("ls / 执行失败: {}", e);
        let _ = std::fs::remove_file(crate::config::Config::save_path());
        return Err(anyhow::anyhow!("Token 校验失败，可能已过期: {}", e));
    }

    success!("Token 验证成功!");
    debug!("存储类型: {}", storage_type);
    success!(
        "配置文件已保存到: {}",
        crate::config::Config::save_path().display()
    );

    Ok(config)
}
