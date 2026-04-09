use crate::commands::rename::RenameArgs as OldRenameArgs;
use clap::Parser;
use std::convert::From;

#[derive(Parser, Debug, Clone)]
pub struct RenameArgs {
    #[arg(help = "源文件路径")]
    pub source: String,

    #[arg(help = "新名称")]
    pub target: String,
}

impl From<RenameArgs> for OldRenameArgs {
    fn from(args: RenameArgs) -> Self {
        OldRenameArgs {
            source: args.source,
            target: args.target,
        }
    }
}
