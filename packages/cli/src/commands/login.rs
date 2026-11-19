use crate::cli::LoginArgs;
use std::path::Path;

pub(super) async fn execute(args: LoginArgs, path: &Path) -> anyhow::Result<()> {
    let config = cloud139::services::login::login(&args.token).await?;
    crate::config::save_config(&config, path)?;
    crate::success!("Token 验证成功!");
    crate::success!("配置文件已保存到: {}", path.display());
    Ok(())
}
