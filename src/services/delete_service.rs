use super::directory_service as dirs;
use crate::{
    client::{ClientError, api},
    config::Config,
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
    let client = api::HttpClientWrapper::new();
    let personal = api::PersonalClient::connect(config, &client, api::ListingRetry::None).await?;
    let (entry, _) = dirs::resolve_entry(&personal, &path).await?;
    if entry.folder && !recursive {
        return Err(dirs::invalid("删除目录需要显式指定 -r / --recursive").into());
    }
    trash_personal_file_id(&personal, &entry.id).await?;
    Ok(())
}

pub(super) async fn trash_personal_file_id(
    personal: &api::PersonalClient<'_>,
    file_id: &str,
) -> Result<(), ClientError> {
    let response: serde_json::Value = personal
        .request(
            "/recyclebin/batchTrash",
            serde_json::json!({ "fileIds": [file_id] }),
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
