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
    config.require_personal()?;
    let path = dirs::non_root(path)?;
    if permanent {
        return Err(dirs::invalid("尚无可靠的永久删除协议，已拒绝操作；未移动到回收站").into());
    }
    let entry = dirs::resolve(config, &path).await?;
    if entry.folder && !recursive {
        return Err(dirs::invalid("删除目录需要显式指定 -r / --recursive").into());
    }
    let mut cached = config.clone();
    let host = api::get_personal_cloud_host(&mut cached).await?;
    trash_personal_file_id(&cached, &host, &entry.id).await?;
    Ok(())
}

pub(super) async fn trash_personal_file_id(
    config: &Config,
    host: &str,
    file_id: &str,
) -> Result<(), ClientError> {
    let response: serde_json::Value = api::personal_api_request(
        config,
        &format!("{host}/recyclebin/batchTrash"),
        serde_json::json!({ "fileIds": [file_id] }),
        StorageType::PersonalNew,
    )
    .await?;
    if !dirs::mutation_succeeded(&response) {
        return Err(ClientError::Api(
            response["message"]
                .as_str()
                .filter(|message| !message.is_empty())
                .unwrap_or("删除失败：服务端未确认全部条目已移到回收站")
                .to_owned(),
        ));
    }
    Ok(())
}
