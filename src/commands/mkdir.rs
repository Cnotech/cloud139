use crate::services::mkdir_service::{MkdirMode, mkdir};
use clap::Parser;

#[derive(Parser, Debug, Clone)]
pub struct MkdirArgs {
    #[arg(short = 'p', long, help = "递归创建父目录；已有目录成功，同名文件报错")]
    pub parents: bool,

    #[arg(help = "新目录路径，格式: /父目录/新目录名")]
    pub path: String,

    #[arg(short, long, help = "兼容选项；目录重名仍报错，不自动改名")]
    pub force: bool,
}

pub async fn execute(args: MkdirArgs) -> anyhow::Result<()> {
    let config = crate::config::Config::load_personal()?;
    let mode = if args.parents {
        MkdirMode::Parents
    } else {
        MkdirMode::Single
    };
    mkdir(&config, &args.path, mode).await?;
    Ok(())
}
