use crate::commands::delete::DeleteArgs as OldDeleteArgs;
use clap::Parser;
use std::convert::From;

#[derive(Parser, Debug, Clone)]
pub struct DeleteArgs {
    #[arg(help = "远程文件路径")]
    pub path: String,

    #[arg(short, long, help = "确认删除")]
    pub yes: bool,

    #[arg(short, long, help = "永久删除（不移动到回收站）")]
    pub permanent: bool,
}

impl From<DeleteArgs> for OldDeleteArgs {
    fn from(args: DeleteArgs) -> Self {
        OldDeleteArgs {
            path: args.path,
            yes: args.yes,
            permanent: args.permanent,
        }
    }
}
