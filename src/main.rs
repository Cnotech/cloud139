use clap::{Parser, Subcommand};

use cloud139::commands::{cp, delete, download, list, login, mkdir, mv, rename, upload};
use cloud139::error;

#[derive(Parser)]
#[command(name = "cloud139")]
#[command(about = "139 Yun CLI - 移动云盘命令行工具", long_about = None)]
#[command(version = "0.1.0")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(short, long, default_value = "info")]
    verbose: String,
}

#[derive(Subcommand)]
enum Commands {
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
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Login(args) => login::execute(args).await,
        Commands::Ls(args) => list::execute(args).await,
        Commands::Upload(args) => upload::execute(args).await,
        Commands::Download(args) => download::execute(args).await,
        Commands::Rm(args) => delete::execute(args).await,
        Commands::Mkdir(args) => mkdir::execute(args).await,
        Commands::Mv(args) => mv::execute(args).await,
        Commands::Cp(args) => cp::execute(args).await,
        Commands::Rename(args) => rename::execute(args).await,
    };

    if let Err(e) = result {
        error!("{}", e);
        std::process::exit(1);
    }

    Ok(())
}
