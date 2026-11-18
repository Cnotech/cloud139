use clap::Parser;

#[derive(Parser, Debug, Clone)]
pub struct MvArgs {
    #[arg(required = true, help = "源文件或目录路径（支持多个，用空格分隔）")]
    pub source: Vec<String>,

    #[arg(help = "必须是已有目标目录")]
    pub target: String,

    #[arg(short, long, help = "个人盘文件同名时强制继续；目录冲突仍报错")]
    pub force: bool,
}

pub async fn execute(args: MvArgs) -> anyhow::Result<()> {
    let config = crate::config::Config::load_personal()?;
    crate::services::move_service::mv(&config, &args.source, &args.target, args.force).await?;
    Ok(())
}
