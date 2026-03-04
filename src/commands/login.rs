use clap::Parser;
use crate::client::ClientError;

#[derive(Parser, Debug)]
pub struct LoginArgs {
    #[arg(short, long, required = true, help = "手机号码")]
    pub username: String,

    #[arg(short, long, required = true, help = "密码")]
    pub password: String,

    #[arg(short, long, required = true, help = "邮箱Cookies (从 mail.139.com 获取，需包含 RMKEY)")]
    pub cookies: String,

    #[arg(short, long, default_value = "personal_new", help = "存储类型: personal_new, family, group")]
    pub storage_type: String,
}

pub async fn execute(args: LoginArgs) -> Result<(), ClientError> {
    println!("正在登录用户: {} ...", args.username);

    let config = crate::client::auth::login(
        &args.username,
        &args.password,
        &args.cookies,
        &args.storage_type,
    ).await?;

    config.save()?;

    println!("登录成功!");
    println!("存储类型: {}", args.storage_type);
    println!("配置文件已保存到: ./config/config.json");

    Ok(())
}
