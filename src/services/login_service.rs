use crate::client::api;
use crate::config::Config;
use crate::domain::StorageType;
use crate::{debug, info, success};

pub async fn login(
    token: &str,
    storage_type: &str,
    cloud_id: Option<&str>,
) -> anyhow::Result<Config> {
    login_with_group_root(token, storage_type, cloud_id, None).await
}

pub async fn login_with_group_root(
    token: &str,
    storage_type: &str,
    cloud_id: Option<&str>,
    root_folder_id: Option<&str>,
) -> anyhow::Result<Config> {
    let token = token.strip_prefix("Basic ").unwrap_or(token);
    let mut config = crate::client::auth::login(token, storage_type, cloud_id).await?;
    let path = Config::save_path();
    if config.storage_type() == StorageType::Group {
        config.root_folder_id = match root_folder_id {
            Some(root) if !root.trim().is_empty() => Some(root.to_owned()),
            Some(_) => anyhow::bail!("--root-folder-id 不能为空"),
            None => Config::load_from_candidates(std::slice::from_ref(&path))
                .ok()
                .map(|(old, _)| old)
                .filter(|old| {
                    old.storage_type() == StorageType::Group
                        && old.account == config.account
                        && old.cloud_id == config.cloud_id
                })
                .and_then(|old| old.root_folder_id),
        };
        super::legacy_directory::root(&config)?;
    } else if root_folder_id.is_some() {
        anyhow::bail!("--root-folder-id 仅用于群组盘登录");
    }

    info!("正在校验 Token 可用性 (ls /) ...");
    if config.storage_type() == StorageType::PersonalNew {
        config.personal_cloud_host =
            Some(api::fetch_personal_cloud_host(&config, &api::HttpClientWrapper::new()).await?);
    }
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
    debug!("存储类型: {}", storage_type);
    success!("配置文件已保存到: {}", path.display());

    Ok(config)
}
