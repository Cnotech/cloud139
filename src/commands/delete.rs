use crate::client::ClientError;
use crate::{debug, info, success, warn};
use clap::Parser;

#[derive(Parser, Debug)]
pub struct DeleteArgs {
    #[arg(short = 'r', long, help = "确认递归删除目录（仍需 --yes）")]
    pub recursive: bool,

    #[arg(help = "远程文件或目录路径")]
    pub path: String,

    #[arg(short, long, help = "确认删除")]
    pub yes: bool,
}

pub async fn execute(args: DeleteArgs) -> anyhow::Result<()> {
    crate::services::directory_service::non_root(&args.path)?;
    if !args.yes {
        warn!("此操作会将文件或目录移动到回收站");
        info!("使用 --yes 参数确认删除；目录还需要 -r");
        return Err(ClientError::ConfirmationRequired.into());
    }

    let config = crate::config::Config::load_personal()?;

    debug!("准备删除云端条目 path={}", args.path);
    crate::services::delete(&config, &args.path, args.recursive).await?;

    success!("文件或目录已移动到回收站");

    Ok(())
}
