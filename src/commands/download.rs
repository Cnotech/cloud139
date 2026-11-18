use crate::config::Config;
use crate::{debug, step, success};
use clap::Parser;

#[derive(Parser, Debug)]
pub struct DownloadArgs {
    #[arg(short = 'r', long, help = "递归下载目录，保留空目录")]
    pub recursive: bool,

    #[arg(help = "远程文件或目录路径")]
    pub remote_path: String,

    #[arg(help = "本地保存路径（默认保存到当前目录的同名文件）")]
    pub local_path: Option<String>,
}

pub async fn execute(args: DownloadArgs) -> anyhow::Result<()> {
    execute_with_config(args, &Config::load_personal()?).await
}

pub(super) async fn execute_with_config(args: DownloadArgs, config: &Config) -> anyhow::Result<()> {
    let normalized = crate::services::directory_service::normalize(&args.remote_path)?;
    let remote_path = &normalized;
    let local_path = crate::utils::resolve_local_path(remote_path, &args.local_path);

    debug!("准备下载文件 remote={} local={}", remote_path, local_path);
    step!("开始下载到: {:?}", local_path);

    crate::services::download(config, remote_path, &local_path, None, args.recursive).await?;

    success!("下载完成!");
    Ok(())
}
