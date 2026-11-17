use super::directory_service as dirs;
use crate::{client::ClientError, config::Config};
pub async fn mkdir(config: &Config, path: &str, _force: bool) -> Result<(), ClientError> {
    dirs::ensure_dir(config, path, false).await
}
pub async fn mkdir_parents(config: &Config, path: &str) -> Result<(), ClientError> {
    dirs::ensure_dir(config, path, true).await
}
pub fn parse_path(path: &str) -> Result<(String, String), ClientError> {
    if path.trim().is_empty() {
        return Err(dirs::invalid("路径不能为空"));
    }
    let path = dirs::non_root(path.trim())?;
    let (parent, name) = path.rsplit_once('/').unwrap();
    Ok((
        if parent.is_empty() {
            "/".into()
        } else {
            parent.into()
        },
        name.into(),
    ))
}
