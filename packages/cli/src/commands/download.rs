use crate::cli::DownloadArgs;
use crate::success;
use cloud139::config::Config;
use cloud139::services::download::download_path;

pub(super) async fn execute(args: DownloadArgs, config: &Config) -> anyhow::Result<()> {
    download_path(config, &args.remote_path, &args.local_path, args.recursive).await?;
    success!("下载完成!");
    Ok(())
}
