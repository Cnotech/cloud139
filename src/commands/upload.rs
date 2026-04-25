use clap::Parser;
use indicatif::MultiProgress;

#[derive(Parser, Debug, Clone)]
pub struct UploadArgs {
    #[arg(help = "本地文件路径")]
    pub local_path: String,

    #[arg(default_value = "/", help = "远程目录路径")]
    pub remote_path: String,

    #[arg(short, long, help = "强制继续，如果云端存在同名文件则自动重命名")]
    pub force: bool,
}

pub async fn execute(args: UploadArgs) -> anyhow::Result<()> {
    let config = crate::config::Config::load()?;

    let local_path = std::path::Path::new(&args.local_path);
    if !local_path.exists() {
        crate::error!("文件不存在: {}", args.local_path);
        return Err(crate::client::ClientError::FileNotFound.into());
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

    crate::debug!("上传文件: {} -> {}/{}", args.local_path, remote_dir, file_name);
    crate::debug!("文件大小: {} bytes", file_size);

    let mp = MultiProgress::new();
    let pb = crate::presentation::make_upload_progress(&mp, file_name, file_size as u64);

    crate::application::services::upload_service::upload(
        &config,
        local_path,
        &remote_dir,
        file_name,
        file_size,
        args.force,
        pb,
    )
    .await?;

    Ok(())
}
