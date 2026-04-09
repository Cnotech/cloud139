use crate::commands::login::LoginArgs as OldLoginArgs;
use clap::Parser;
use std::convert::From;

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
}

impl From<LoginArgs> for OldLoginArgs {
    fn from(args: LoginArgs) -> Self {
        OldLoginArgs {
            token: args.token,
            storage_type: args.storage_type,
            cloud_id: None,
        }
    }
}
