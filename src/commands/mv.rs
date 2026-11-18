use crate::config::Config;
use clap::Parser;

#[derive(Parser, Debug, Clone)]
pub struct MvArgs {
    #[arg(required = true, help = "源文件或目录路径（支持多个，用空格分隔）")]
    pub source: Vec<String>,

    #[arg(help = "必须是已有目标目录")]
    pub target: String,

    #[arg(
        short,
        long,
        help = "目标目录已有同名文件时，仍提交移动。涉及目录的同名冲突会报错"
    )]
    pub force: bool,
}

pub async fn execute(args: MvArgs) -> anyhow::Result<()> {
    execute_with_config(args, &Config::load_personal()?).await
}

pub(super) async fn execute_with_config(args: MvArgs, config: &Config) -> anyhow::Result<()> {
    crate::services::move_service::mv(config, &args.source, &args.target, args.force).await?;
    Ok(())
}
