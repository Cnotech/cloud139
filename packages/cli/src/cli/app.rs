use clap::{Parser, Subcommand};
use std::path::PathBuf;

use super::args::{
    CpArgs, DeleteArgs, DownloadArgs, ListArgs, LoginArgs, MkdirArgs, MvArgs, RenameArgs, SyncArgs,
    UploadArgs,
};

#[derive(Parser)]
#[command(name = "cloud139")]
#[command(about = "中国移动云盘命令行工具", long_about = None)]
#[command(version)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Commands,

    #[arg(short, long, default_value = "info")]
    pub(crate) verbose: String,

    /// 指定配置文件路径 (覆盖默认查找顺序)
    #[arg(short, long, global = true, value_name = "PATH")]
    pub(crate) config: Option<PathBuf>,
}

#[derive(Subcommand)]
pub(crate) enum Commands {
    /// 登录账号
    Login(LoginArgs),
    /// 列出文件和目录
    Ls(ListArgs),
    /// 上传文件和目录
    Upload(UploadArgs),
    /// 下载文件和目录
    Download(DownloadArgs),
    /// 删除文件和目录
    Rm(DeleteArgs),
    /// 创建目录
    Mkdir(MkdirArgs),
    /// 移动文件和目录
    Mv(MvArgs),
    /// 复制文件和目录
    Cp(CpArgs),
    /// 重命名文件和目录
    Rename(RenameArgs),
    /// 同步本地目录和云端目录
    Sync(SyncArgs),
}
