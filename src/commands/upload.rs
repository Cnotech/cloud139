use crate::config::Config;
use clap::Parser;
use indicatif::MultiProgress;

#[derive(Parser, Debug, Clone)]
#[command(
    after_help = "单个文件的上传大小上限因账号而异。上传目录时，单个文件失败后仍会处理其余文件。有文件上传失败时，退出码为 1。"
)]
pub struct UploadArgs {
    #[arg(help = "本地文件或目录；目录末尾 / 表示仅上传内容，否则包含目录本身")]
    pub local_path: String,

    #[arg(default_value = "/", help = "远程目录路径")]
    pub remote_path: String,

    #[arg(
        short,
        long,
        help = "允许上传同名文件，由云端为新文件分配名称。文件与目录同名时会报错"
    )]
    pub force: bool,
}

pub async fn execute(args: UploadArgs) -> anyhow::Result<()> {
    execute_with_config(args, &Config::load_personal()?).await
}

pub(super) async fn execute_with_config(args: UploadArgs, config: &Config) -> anyhow::Result<()> {
    let local_path = std::path::Path::new(&args.local_path);
    if !local_path.exists() {
        crate::error!("文件不存在: {}", args.local_path);
        return Err(crate::client::ClientError::FileNotFound.into());
    }

    crate::services::directory_service::safe_local(local_path)?;
    if local_path.is_dir() {
        return crate::services::upload_service::upload_directory(
            config,
            local_path,
            &args.remote_path,
            args.local_path.ends_with('/') || args.local_path.ends_with('\\'),
            args.force,
        )
        .await;
    }
    if !local_path.is_file() {
        anyhow::bail!("只支持普通文件或目录");
    }
    let file_name = local_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    let file_size = crate::services::upload_service::checked_upload_size(local_path)
        .map_err(|error| anyhow::anyhow!("无法上传 {}: {}", args.local_path, error))?;

    let remote_dir = if args.remote_path == "/" {
        "".to_string()
    } else {
        args.remote_path.trim_end_matches('/').to_string()
    };

    crate::debug!(
        "准备上传文件 local={} remote={}/{}",
        args.local_path,
        remote_dir,
        file_name
    );
    crate::debug!("已读取本地文件大小 size_bytes={}", file_size);

    let mp = MultiProgress::new();
    let pb = crate::presentation::make_upload_progress(&mp, file_name, file_size as u64);

    crate::services::upload_service::upload(
        config,
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
