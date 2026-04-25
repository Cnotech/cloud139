use clap::Parser;

#[derive(Parser, Debug, Clone)]
pub struct MvArgs {
    #[arg(required = true, help = "源文件路径（支持多个，用空格分隔）")]
    pub source: Vec<String>,

    #[arg(help = "目标路径")]
    pub target: String,

    #[arg(short, long, help = "强制继续，如果云端存在同名文件则自动重命名")]
    pub force: bool,
}

pub async fn execute(args: MvArgs) -> anyhow::Result<()> {
    let config = crate::commands::dispatch::load_config()?;
    crate::application::services::move_service::mv(
        &config,
        &args.source,
        &args.target,
        args.force,
    )
    .await?;
    Ok(())
}
