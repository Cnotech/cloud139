use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use std::io::IsTerminal;

use crate::client::StorageType;

pub mod family;
pub mod group;
pub mod personal;
pub mod personal_parts;

pub use crate::client::ClientError;

#[derive(Parser, Debug)]
pub struct UploadArgs {
    #[arg(help = "本地文件路径")]
    pub local_path: String,

    #[arg(default_value = "/", help = "远程目录路径")]
    pub remote_path: String,

    #[arg(short, long, help = "强制继续，如果云端存在同名文件则自动重命名")]
    pub force: bool,
}

/// 上传分片参数
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

fn make_upload_progress(file_name: &str, file_size: u64) -> Option<ProgressBar> {
    if !std::io::stderr().is_terminal() {
        return None;
    }
    let pb = ProgressBar::new(file_size);
    let style = ProgressStyle::with_template(
        "{msg} {bar:24.cyan/blue} {bytes}/{total_bytes} {bytes_per_sec} {eta}",
    )
    .unwrap_or_else(|_| ProgressStyle::default_bar());
    pb.set_style(style);
    pb.set_message(file_name.to_string());
    Some(pb)
}

pub async fn execute(args: UploadArgs) -> anyhow::Result<()> {
    let config = crate::commands::dispatch::load_config()?;
    let storage_type = config.storage_type();

    let local_path = std::path::Path::new(&args.local_path);
    if !local_path.exists() {
        crate::error!("文件不存在: {}", args.local_path);
        return Err(ClientError::FileNotFound.into());
    }

    let file_name = local_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    let metadata = std::fs::metadata(local_path)?;
    let file_size = metadata.len() as i64;

    let remote_dir = if args.remote_path == "/" {
        "".to_string()
    } else {
        args.remote_path.trim_end_matches('/').to_string()
    };

    crate::info!(
        "上传文件: {} -> {}/{}",
        args.local_path,
        remote_dir,
        file_name
    );
    crate::debug!("文件大小: {} bytes", file_size);

    let pb = make_upload_progress(file_name, file_size as u64);

    match storage_type {
        StorageType::PersonalNew => {
            personal::upload(
                &config,
                local_path,
                &remote_dir,
                file_name,
                file_size,
                args.force,
                pb,
            )
            .await?
        }
        StorageType::Family => {
            family::upload(&config, local_path, &remote_dir, file_name, file_size, pb).await?
        }
        StorageType::Group => {
            group::upload(&config, local_path, &remote_dir, file_name, file_size, pb).await?
        }
    }

    Ok(())
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
