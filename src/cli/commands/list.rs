use crate::commands::list::ListArgs as OldListArgs;
use clap::Parser;
use std::convert::From;

#[derive(Parser, Debug, Clone)]
pub struct ListArgs {
    #[arg(default_value = "/", help = "远程目录路径")]
    pub path: String,

    #[arg(short, long, default_value = "1", help = "页码")]
    pub page: i32,

    #[arg(short = 's', long, default_value = "100", help = "每页数量")]
    pub page_size: i32,

    #[arg(short, long, help = "将JSON输出到指定文件")]
    pub output: Option<String>,
}

impl From<ListArgs> for OldListArgs {
    fn from(args: ListArgs) -> Self {
        OldListArgs {
            path: args.path,
            page: args.page,
            page_size: args.page_size,
            output: args.output,
        }
    }
}
