use clap::Parser;
use cloud139_core::config::Config;

#[derive(Parser, Debug, Clone)]
pub struct CpArgs {
    #[arg(required = true, help = "源文件或目录路径（支持多个，用空格分隔）")]
    pub source: Vec<String>,

    #[arg(help = "目标目录")]
    pub target: String,

    #[arg(
        short,
        long,
        help = "目标目录已有同名文件时，仍提交复制。涉及目录的同名冲突会报错"
    )]
    pub force: bool,
}

pub(super) async fn execute(args: CpArgs, config: &Config) -> anyhow::Result<()> {
    cloud139_core::services::copy_service::cp(config, &args.source, &args.target, args.force)
        .await?;
    Ok(())
}
