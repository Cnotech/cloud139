use crate::success;
use clap::Parser;
use cloud139_core::config::Config;
use cloud139_core::services::download_service::download_path;

#[derive(Parser, Debug)]
pub struct DownloadArgs {
    #[arg(short = 'r', long, help = "递归下载目录，保留空目录")]
    pub recursive: bool,

    #[arg(help = "远程文件或目录路径")]
    pub remote_path: String,

    #[arg(help = "本地保存路径（默认保存到当前目录的同名文件）")]
    pub local_path: Option<String>,
}

pub(super) async fn execute(args: DownloadArgs, config: &Config) -> anyhow::Result<()> {
    download_path(config, &args.remote_path, &args.local_path, args.recursive).await?;
    success!("下载完成!");
    Ok(())
}
