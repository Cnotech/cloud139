use clap::Parser;
use cloud139::config::Config;
use cloud139::services::mkdir_service::{MkdirMode, mkdir};

#[derive(Parser, Debug, Clone)]
pub struct MkdirArgs {
    #[arg(short = 'p', long, help = "递归创建父目录；已有目录成功，同名文件报错")]
    pub parents: bool,

    #[arg(help = "新目录路径，格式: /父目录/新目录名")]
    pub path: String,
}

pub(super) async fn execute(args: MkdirArgs, config: &Config) -> anyhow::Result<()> {
    let mode = if args.parents {
        MkdirMode::Parents
    } else {
        MkdirMode::Single
    };
    mkdir(config, &args.path, mode).await?;
    Ok(())
}
