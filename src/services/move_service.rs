use crate::{client::ClientError, config::Config, domain::StorageType};
pub async fn mv(
    config: &Config,
    sources: &[String],
    target: &str,
    force: bool,
) -> Result<(), ClientError> {
    if sources.len() > 1 {
        match config.storage_type() {
            StorageType::Family => return Err(ClientError::UnsupportedFamilyBatchMove),
            StorageType::Group => return Err(ClientError::UnsupportedGroupBatchMove),
            _ => {}
        }
    }
    super::directory_service::transfer(config, sources, target, force, true).await
}
