use clap::{Parser, Subcommand};
use std::path::PathBuf;

use crate::cli::commands::{cp, delete, download, list, login, mkdir, mv, rename, sync, upload};

#[derive(Parser)]
#[command(name = "cloud139")]
#[command(about = "139 Yun CLI - 移动云盘命令行工具", long_about = None)]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    #[arg(short, long, default_value = "info")]
    pub verbose: String,

    /// 指定配置文件路径 (覆盖默认查找顺序)
    #[arg(short, long, global = true, value_name = "PATH")]
    pub config: Option<PathBuf>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// 登录账号
    Login(login::LoginArgs),
    /// 列出文件
    Ls(list::ListArgs),
    /// 上传文件
    Upload(upload::UploadArgs),
    /// 下载文件
    Download(download::DownloadArgs),
    /// 删除文件
    Rm(delete::DeleteArgs),
    /// 创建目录
    Mkdir(mkdir::MkdirArgs),
    /// 移动文件
    Mv(mv::MvArgs),
    /// 复制文件
    Cp(cp::CpArgs),
    /// 重命名文件
    Rename(rename::RenameArgs),
    /// 同步本地目录和云端目录
    Sync(sync::SyncArgs),
}
