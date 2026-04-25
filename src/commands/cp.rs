use clap::Parser;

#[derive(Parser, Debug, Clone)]
pub struct CpArgs {
    #[arg(help = "源文件路径")]
    pub source: String,

    #[arg(help = "目标目录")]
    pub target: String,

    #[arg(short, long, help = "强制继续，如果云端存在同名文件则自动重命名")]
    pub force: bool,
}

pub async fn execute(args: CpArgs) -> anyhow::Result<()> {
    let config = crate::commands::dispatch::load_config()?;
    crate::application::services::copy_service::cp(
        &config,
        &args.source,
        &args.target,
        args.force,
    )
    .await?;
    Ok(())
}
