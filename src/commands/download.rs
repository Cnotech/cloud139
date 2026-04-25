use crate::{debug, step, success};
use clap::Parser;

#[derive(Parser, Debug)]
pub struct DownloadArgs {
    #[arg(help = "远程文件路径")]
    pub remote_path: String,

    #[arg(help = "本地保存路径（默认保存到当前目录的同名文件）")]
    pub local_path: Option<String>,
}

pub async fn execute(args: DownloadArgs) -> anyhow::Result<()> {
    let config = crate::config::Config::load()?;

    let remote_path = &args.remote_path;
    let local_path = crate::utils::resolve_local_path(remote_path, &args.local_path);

    debug!("下载: remote={}, local={}", remote_path, local_path);
    step!("开始下载到: {:?}", local_path);

    let pb = crate::presentation::renderers::make_download_progress(remote_path);
    crate::application::services::download(&config, remote_path, &local_path, pb).await?;

    success!("下载完成!");
    Ok(())
}
