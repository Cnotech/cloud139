use crate::commands::mv::MvArgs as OldMvArgs;
use clap::Parser;
use std::convert::From;

#[derive(Parser, Debug, Clone)]
pub struct MvArgs {
    #[arg(required = true, help = "源文件路径（支持多个，用空格分隔）")]
    pub source: Vec<String>,

    #[arg(help = "目标路径")]
    pub target: String,

    #[arg(short, long, help = "强制继续，如果云端存在同名文件则自动重命名")]
    pub force: bool,
}

impl From<MvArgs> for OldMvArgs {
    fn from(args: MvArgs) -> Self {
        OldMvArgs {
            source: args.source,
            target: args.target,
            force: args.force,
        }
    }
}
