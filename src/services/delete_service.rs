use super::directory_service as dirs;
use crate::{client::api, config::Config};
pub async fn delete(
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
    personal.trash(&entry.id).await?;
    Ok(())
}
