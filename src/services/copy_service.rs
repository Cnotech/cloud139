use crate::{client::ClientError, config::Config, domain::StorageType};
pub async fn cp(
    config: &Config,
    sources: &[String],
    target: &str,
    force: bool,
) -> Result<(), ClientError> {
    if sources.len() > 1 {
        match config.storage_type() {
            StorageType::Family => return Err(ClientError::UnsupportedFamilyBatchCopy),
            StorageType::Group => return Err(ClientError::UnsupportedGroupBatchCopy),
            _ => {}
        }
    }
    super::directory_service::transfer(config, sources, target, force, false).await
}
