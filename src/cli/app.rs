use clap::{Parser, Subcommand};

use crate::cli::commands::{download, list, upload};

#[derive(Parser)]
#[command(name = "cloud139")]
#[command(about = "139 Yun CLI - 移动云盘命令行工具", long_about = None)]
#[command(version = "0.1.0")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    #[arg(short, long, default_value = "info")]
    pub verbose: String,
}

#[derive(Subcommand)]
pub enum Commands {
    Ls(list::ListArgs),
    Upload(upload::UploadArgs),
    Download(download::DownloadArgs),
}
