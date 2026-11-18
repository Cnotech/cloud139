use crate::services::mkdir_service::{MkdirMode, mkdir};
use clap::Parser;

#[derive(Parser, Debug, Clone)]
pub struct MkdirArgs {
    #[arg(short = 'p', long, help = "递归创建父目录；已有目录成功，同名文件报错")]
    pub parents: bool,

    #[arg(help = "新目录路径，格式: /父目录/新目录名")]
    pub path: String,

    #[arg(short, long, help = "为旧脚本保留。传入后与省略时行为相同")]
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
