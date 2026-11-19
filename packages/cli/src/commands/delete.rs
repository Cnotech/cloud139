use crate::{debug, info, success, warn};
use clap::Parser;
use cloud139_core::client::ClientError;
use cloud139_core::config::Config;

#[derive(Parser, Debug)]
pub struct DeleteArgs {
    #[arg(short = 'r', long, help = "确认递归删除目录（仍需 --yes）")]
    pub recursive: bool,

    #[arg(help = "远程文件或目录路径")]
    pub path: String,

    #[arg(short, long, help = "确认删除")]
    pub yes: bool,
}

pub(super) fn preflight(args: &DeleteArgs) -> anyhow::Result<()> {
    cloud139_core::services::directory_service::non_root(&args.path)?;
    if !args.yes {
        warn!("此操作会将文件或目录移动到回收站");
        info!("使用 --yes 参数确认删除；目录还需要 -r");
        return Err(ClientError::ConfirmationRequired.into());
    }
    Ok(())
}

pub(super) async fn execute(args: DeleteArgs, config: &Config) -> anyhow::Result<()> {
    debug!("准备删除云端条目 path={}", args.path);
    cloud139_core::services::delete(config, &args.path, args.recursive).await?;

    success!("文件或目录已移动到回收站");

    Ok(())
}
