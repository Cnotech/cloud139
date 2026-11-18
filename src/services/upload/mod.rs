pub mod personal;
pub mod personal_parts;

pub mod family {
    pub use super::unsupported::upload;
}

pub mod group {
    pub use super::unsupported::upload;
}

mod unsupported {
    use crate::{client::ClientError, config::Config};
    use indicatif::ProgressBar;
    use std::path::Path;

    pub async fn upload(
        _config: &Config,
        _local_path: &Path,
        _remote_path: &str,
        _file_name: &str,
        _file_size: i64,
        _progress: Option<ProgressBar>,
    ) -> Result<(), ClientError> {
        Err(ClientError::UnsupportedStorage)
    }
}
