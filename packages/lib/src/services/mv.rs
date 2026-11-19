use crate::{config::Config, error::ClientError};
pub async fn mv(
    config: &Config,
    sources: &[String],
    target: &str,
    force: bool,
) -> Result<(), ClientError> {
    super::directory::transfer(config, sources, target, force, true).await
}
