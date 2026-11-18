use crate::{client::ClientError, config::Config};
pub async fn mv(
    config: &Config,
    sources: &[String],
    target: &str,
    force: bool,
) -> Result<(), ClientError> {
    super::directory_service::transfer(config, sources, target, force, true).await
}
