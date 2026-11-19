use clap::Parser;
use std::path::Path;

#[derive(Parser, Debug, Clone)]
pub struct LoginArgs {
    #[arg(
        short,
        long,
        required = true,
        help = "Authorization Token (从浏览器开发者工具获取)"
    )]
    pub token: String,
}

pub(super) async fn execute(args: LoginArgs, path: &Path) -> anyhow::Result<()> {
    let config = cloud139::services::login_service::login(&args.token).await?;
    crate::config::save_config(&config, path)?;
    crate::success!("Token 验证成功!");
    crate::success!("配置文件已保存到: {}", path.display());
    Ok(())
}
