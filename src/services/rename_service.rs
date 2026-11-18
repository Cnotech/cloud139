use super::directory_service as dirs;
use crate::{
    client::{ClientError, api},
    config::Config,
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
    let client = api::HttpClientWrapper::new();
    let personal = api::PersonalClient::connect(config, &client, api::ListingRetry::None).await?;
    let (entry, _) = dirs::resolve_entry(&personal, &source).await?;
    let (parent, name) = source.rsplit_once('/').unwrap();
    if name == target {
        return Ok(());
    }
    let parent_id = dirs::directory_with_listing(&personal, parent).await?;
    if let Some(conflict) = personal
        .list_files(&parent_id)
        .await?
        .iter()
        .find(|item| item.name.as_deref() == Some(target))
        && (entry.folder || conflict.is_folder() || !force)
    {
        return Err(ClientError::ForceRequired);
    }
    rename_personal_id(&personal, &entry.id, target).await
}

pub(super) async fn rename_personal_id(
    personal: &api::PersonalClient<'_>,
    id: &str,
    name: &str,
) -> Result<(), ClientError> {
    let resp: crate::models::PersonalUploadResp = personal
        .request(
            "/file/update",
            serde_json::json!({ "fileId": id, "name": name, "description": "" }),
        )
        .await?;
    if !resp.base.success {
        return Err(ClientError::Api(
            resp.base.message.unwrap_or_else(|| "重命名失败".into()),
        ));
    }
    Ok(())
}
