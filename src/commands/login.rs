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

    #[arg(
        short,
        long,
        default_value = "personal_new",
        help = "存储类型: personal_new, family, group"
    )]
    pub storage_type: String,

    #[arg(long, help = "云盘ID (家庭云/群组云时需要)")]
    pub cloud_id: Option<String>,
}

pub async fn execute(args: LoginArgs) -> anyhow::Result<()> {
    crate::services::login_service::login(
        &args.token,
        &args.storage_type,
        args.cloud_id.as_deref(),
    )
    .await?;
    Ok(())
}
