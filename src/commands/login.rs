use clap::Parser;
use crate::client::ClientError;
use crate::info;
use crate::success;

#[derive(Parser, Debug)]
pub struct LoginArgs {
    #[arg(short, long, required = true, help = "Authorization Token (从浏览器开发者工具获取)")]
    pub token: String,

    #[arg(short, long, default_value = "personal_new", help = "存储类型: personal_new, family, group")]
    pub storage_type: String,

    #[arg(short, long, help = "云盘ID (家庭云/和家亲时需要)")]
    pub cloud_id: Option<String>,
}

pub async fn execute(args: LoginArgs) -> Result<(), ClientError> {
    info!("正在验证 Token ...");

    let token=args.token.strip_prefix("Basic ").map(|s| s.to_string()).unwrap_or_else(||args.token);
    
    let config = crate::client::auth::login(
        &token,
        &args.storage_type,
        args.cloud_id.as_deref(),
    ).await?;

    config.save()?;

    success!("Token 验证成功!");
    info!("存储类型: {}", args.storage_type);
    success!("配置文件已保存到: ./config/config.json");

    Ok(())
}
