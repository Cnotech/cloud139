use crate::client::ClientError;
use crate::domain::StorageType;
use indicatif::ProgressBar;
use std::path::Path;

pub struct UploadPartParams<'a> {
    pub upload_url: &'a str,
    pub upload_task_id: &'a str,
    pub buffer: &'a [u8],
    pub part_number: i64,
    pub part_offset: i64,
    pub read_size: i64,
    pub file_name: &'a str,
    pub total_size: i64,
}

pub fn get_part_size(size: i64, custom_size: i64) -> i64 {
    if custom_size != 0 {
        return custom_size;
    }
    if size / (1024 * 1024 * 1024) > 30 {
        return 512 * 1024 * 1024;
    }
    100 * 1024 * 1024
}

pub async fn upload(
    config: &crate::config::Config,
    local_path: &Path,
    remote_path: &str,
    file_name: &str,
    file_size: i64,
    force: bool,
    pb: Option<ProgressBar>,
) -> Result<(), ClientError> {
    let storage_type = config.storage_type();
    match storage_type {
        StorageType::PersonalNew => {
            crate::services::upload::personal::upload(
                config, local_path, remote_path, file_name, file_size, force, pb,
            )
            .await
        }
        StorageType::Family => {
            crate::services::upload::family::upload(
                config, local_path, remote_path, file_name, file_size, pb,
            )
            .await
        }
        StorageType::Group => {
            crate::services::upload::group::upload(
                config, local_path, remote_path, file_name, file_size, pb,
            )
            .await
        }
    }
}
