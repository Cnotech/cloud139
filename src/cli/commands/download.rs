use crate::commands::download::DownloadArgs as OldDownloadArgs;
use clap::Parser;

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
