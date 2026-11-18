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
        help = "存储类型（仅实现 personal_new）"
    )]
    pub storage_type: String,

    #[arg(long, help = "把云盘 ID 写入配置。个人云根据 Token 识别")]
    pub cloud_id: Option<String>,

    #[arg(long, help = "群组盘根目录 ID。当前传入会报错")]
    pub root_folder_id: Option<String>,
}

pub async fn execute(args: LoginArgs) -> anyhow::Result<()> {
    crate::services::login_service::login(
        &args.token,
        &args.storage_type,
        args.cloud_id.as_deref(),
        args.root_folder_id.as_deref(),
    )
    .await?;
    Ok(())
}
