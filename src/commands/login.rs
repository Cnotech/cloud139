use clap::Parser;

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

pub async fn execute(args: LoginArgs) -> anyhow::Result<()> {
    crate::services::login_service::login(&args.token).await?;
    Ok(())
}
