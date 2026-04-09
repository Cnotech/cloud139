use crate::commands::mkdir::MkdirArgs as OldMkdirArgs;
use clap::Parser;
use std::convert::From;

#[derive(Parser, Debug, Clone)]
pub struct MkdirArgs {
    #[arg(help = "新目录路径，格式: /父目录/新目录名")]
    pub path: String,

    #[arg(short, long, help = "强制继续，如果云端存在同名目录则自动重命名")]
    pub force: bool,
}

impl From<MkdirArgs> for OldMkdirArgs {
    fn from(args: MkdirArgs) -> Self {
        OldMkdirArgs {
            path: args.path,
            force: args.force,
        }
    }
}
