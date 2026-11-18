use crate::config::Config;
use crate::services::download_service::download_path;
use crate::success;
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
    download_path(config, &args.remote_path, &args.local_path, args.recursive).await?;
    success!("下载完成!");
    Ok(())
}
