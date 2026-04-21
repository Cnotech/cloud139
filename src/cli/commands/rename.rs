use crate::commands::rename::RenameArgs as OldRenameArgs;
use clap::Parser;
use std::convert::From;

#[derive(Parser, Debug, Clone)]
pub struct RenameArgs {
    #[arg(help = "源文件路径")]
    pub source: String,

    #[arg(help = "新名称")]
    pub target: String,

    #[arg(short, long, help = "强制继续，如果云端存在同名文件则自动重命名")]
    pub force: bool,
}

impl From<RenameArgs> for OldRenameArgs {
    fn from(args: RenameArgs) -> Self {
        OldRenameArgs {
            source: args.source,
            target: args.target,
            force: args.force,
        }
    }
}
