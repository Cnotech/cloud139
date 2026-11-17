use clap::Parser;

#[derive(Parser, Debug, Clone)]
pub struct CpArgs {
    #[arg(required = true, help = "源文件或目录路径（支持多个，用空格分隔）")]
    pub source: Vec<String>,

    #[arg(help = "目标目录")]
    pub target: String,

    #[arg(short, long, help = "个人盘文件同名时强制继续；目录冲突仍报错")]
    pub force: bool,
}

pub async fn execute(args: CpArgs) -> anyhow::Result<()> {
    let config = crate::config::Config::load()?;
    crate::services::copy_service::cp(&config, &args.source, &args.target, args.force).await?;
    Ok(())
}
