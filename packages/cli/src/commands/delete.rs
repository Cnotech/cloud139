use crate::cli::args::DeleteArgs;
use crate::{debug, info, success, warn};
use cloud139::client::ClientError;
use cloud139::config::Config;

pub(super) fn preflight(args: &DeleteArgs) -> anyhow::Result<()> {
    cloud139::utils::remote_path::non_root(&args.path)?;
    if !args.yes {
        warn!("此操作会将文件或目录移动到回收站");
        info!("使用 --yes 参数确认删除；目录还需要 -r");
        return Err(ClientError::ConfirmationRequired.into());
    }
    Ok(())
}

pub(super) async fn execute(args: DeleteArgs, config: &Config) -> anyhow::Result<()> {
    debug!("准备删除云端条目 path={}", args.path);
    cloud139::services::delete(config, &args.path, args.recursive).await?;

    success!("文件或目录已移动到回收站");

    Ok(())
}
