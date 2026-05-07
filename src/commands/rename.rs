use clap::Parser;

#[derive(Parser, Debug, Clone)]
pub struct RenameArgs {
    #[arg(help = "源文件路径")]
    pub source: String,

    #[arg(help = "新名称")]
    pub target: String,

    #[arg(short, long, help = "强制继续，如果云端存在同名文件则自动重命名")]
    pub force: bool,
}

pub async fn execute(args: RenameArgs) -> anyhow::Result<()> {
    let config = crate::config::Config::load()?;
    crate::services::rename_service::rename(&config, &args.source, &args.target, args.force)
        .await?;
    Ok(())
}
