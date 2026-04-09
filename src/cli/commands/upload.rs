use crate::commands::download::DownloadArgs as OldDownloadArgs;
use crate::commands::upload::UploadArgs as OldUploadArgs;
use clap::Parser;
use std::convert::From;

#[derive(Parser, Debug, Clone)]
pub struct UploadArgs {
    #[arg(help = "本地文件路径")]
    pub local_path: String,

    #[arg(default_value = "/", help = "远程目录路径")]
    pub remote_path: String,

    #[arg(short, long, help = "强制继续，如果云端存在同名文件则自动重命名")]
    pub force: bool,
}

impl From<UploadArgs> for OldUploadArgs {
    fn from(args: UploadArgs) -> Self {
        OldUploadArgs {
            local_path: args.local_path,
            remote_path: args.remote_path,
            force: args.force,
        }
    }
}

#[derive(Parser, Debug, Clone)]
pub struct DownloadArgs {
    #[arg(help = "远程文件路径")]
    pub remote_path: String,

    #[arg(help = "本地保存路径（默认保存到当前目录的同名文件）")]
    pub local_path: Option<String>,
}

impl From<DownloadArgs> for OldDownloadArgs {
    fn from(args: DownloadArgs) -> Self {
        OldDownloadArgs {
            remote_path: args.remote_path,
            local_path: args.local_path,
        }
    }
}
