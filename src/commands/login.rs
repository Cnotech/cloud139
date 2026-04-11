use crate::info;
use crate::success;
use crate::warn;
use clap::Parser;

#[derive(Parser, Debug)]
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

    #[arg(short, long, help = "云盘ID (家庭云/和家亲时需要)")]
    pub cloud_id: Option<String>,
}

pub async fn execute(args: LoginArgs) -> anyhow::Result<()> {
    let token = args
        .token
        .strip_prefix("Basic ")
        .map(|s| s.to_string())
        .unwrap_or_else(|| args.token);

    let config =
        crate::client::auth::login(&token, &args.storage_type, args.cloud_id.as_deref()).await?;

    config.save()?;

    // 后置校验：执行一次 ls / 确认 Token 实际可用
    info!("正在校验 Token 可用性 (ls /) ...");
    let list_args = crate::cli::commands::list::ListArgs {
        path: "/".to_string(),
        page: 1,
        page_size: 10,
        output: None,
    };
    if let Err(e) = crate::application::services::list(&config, &list_args).await {
        warn!("ls / 执行失败: {}", e);
        return Err(anyhow::anyhow!("Token 校验失败，可能已过期: {}", e));
    }

    success!("Token 验证成功!");
    info!("存储类型: {}", args.storage_type);
    success!("配置文件已保存到: ./cloud139.toml");

    Ok(())
}
