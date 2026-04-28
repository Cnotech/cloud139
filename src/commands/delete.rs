use crate::client::ClientError;
use crate::{debug, info, success, warn};
use clap::Parser;

#[derive(Parser, Debug)]
pub struct DeleteArgs {
    #[arg(help = "远程文件路径")]
    pub path: String,

    #[arg(short, long, help = "确认删除")]
    pub yes: bool,

    #[arg(short, long, help = "永久删除（不移动到回收站）")]
    pub permanent: bool,
}

pub async fn execute(args: DeleteArgs) -> anyhow::Result<()> {
    if !args.yes {
        if args.permanent {
            warn!("此操作将永久删除文件，无法恢复！");
        } else {
            warn!("此操作会将文件移动到回收站");
        }
        info!("使用 --yes 参数确认删除");
        return Err(ClientError::ConfirmationRequired.into());
    }

    let config = crate::config::Config::load()?;

    debug!("delete: path={}, permanent={}", args.path, args.permanent);
    crate::services::delete(&config, &args.path, args.permanent).await?;

    if args.permanent {
        success!("文件已永久删除");
    } else {
        success!("文件已移动到回收站");
    }

    Ok(())
}
