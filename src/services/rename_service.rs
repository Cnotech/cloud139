use super::directory_service as dirs;
use crate::{
    client::{ClientError, api},
    config::Config,
    domain::StorageType,
};
pub async fn rename(
    config: &Config,
    source: &str,
    target: &str,
    force: bool,
) -> Result<(), ClientError> {
    config.require_personal()?;
    let source = dirs::non_root(source)?;
    dirs::validate_name(target)?;
    let entry = dirs::resolve(config, &source).await?;
    let (parent, name) = source.rsplit_once('/').unwrap();
    if name == target {
        return Ok(());
    }
    let parent_id = dirs::directory(config, parent).await?;
    if let Some(conflict) = api::list_personal_files(config, &parent_id)
        .await?
        .iter()
        .find(|i| i.name.as_deref() == Some(target))
        && (entry.folder
            || super::sync_service::should_treat_personal_item_as_folder(conflict)
            || !force)
    {
        return Err(ClientError::ForceRequired);
    }
    let mut cached = config.clone();
    let host = api::get_personal_cloud_host(&mut cached).await?;
    let resp: crate::models::PersonalUploadResp = api::personal_api_request(
        &cached,
        &format!("{host}/file/update"),
        serde_json::json!({ "fileId": entry.id, "name": target, "description": "" }),
        StorageType::PersonalNew,
    )
    .await?;
    if !resp.base.success {
        return Err(ClientError::Api(
            resp.base.message.unwrap_or_else(|| "重命名失败".into()),
        ));
    }
    Ok(())
}
