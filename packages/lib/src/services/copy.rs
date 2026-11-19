use crate::{config::Config, error::ClientError};
pub async fn cp(
    config: &Config,
    sources: &[String],
    target: &str,
    force: bool,
) -> Result<(), ClientError> {
    super::directory::transfer(config, sources, target, force, false).await
}
