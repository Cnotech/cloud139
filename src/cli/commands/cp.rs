use crate::commands::cp::CpArgs as OldCpArgs;
use clap::Parser;
use std::convert::From;

#[derive(Parser, Debug, Clone)]
pub struct CpArgs {
    #[arg(help = "源文件路径")]
    pub source: String,

    #[arg(help = "目标目录")]
    pub target: String,

    #[arg(short, long, help = "强制继续，如果云端存在同名文件则自动重命名")]
    pub force: bool,
}

impl From<CpArgs> for OldCpArgs {
    fn from(args: CpArgs) -> Self {
        OldCpArgs {
            source: args.source,
            target: args.target,
            merge: false,
            force: args.force,
        }
    }
}
