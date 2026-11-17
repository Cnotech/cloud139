use super::directory_service as dirs;
use crate::{
    client::{ClientError, api},
    config::Config,
    domain::StorageType,
};
pub async fn delete(config: &Config, path: &str, permanent: bool) -> anyhow::Result<()> {
    delete_recursive(config, path, permanent, false).await
}
pub async fn delete_recursive(
    config: &Config,
    path: &str,
    permanent: bool,
    recursive: bool,
) -> anyhow::Result<()> {
    let path = dirs::non_root(path)?;
    if permanent {
        return Err(dirs::invalid("尚无可靠的永久删除协议，已拒绝操作；未移动到回收站").into());
    }
    if config.storage_type() != StorageType::PersonalNew {
        return super::legacy_directory::trash(config, &path, recursive)
            .await
            .map_err(Into::into);
    }
    let entry = dirs::resolve(config, &path).await?;
    if entry.folder && !recursive {
        return Err(dirs::invalid("删除目录需要显式指定 -r / --recursive").into());
    }
    let mut cached = config.clone();
    let host = api::get_personal_cloud_host(&mut cached).await?;
    let resp: crate::models::BatchTrashResp = api::personal_api_request(
        &cached,
        &format!("{host}/recyclebin/batchTrash"),
        serde_json::json!({ "fileIds": [entry.id] }),
        StorageType::PersonalNew,
    )
    .await?;
    if !resp.base.success {
        return Err(
            ClientError::Api(resp.base.message.unwrap_or_else(|| "删除失败".into())).into(),
        );
    }
    Ok(())
}
