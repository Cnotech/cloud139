use clap::Parser;

#[derive(Parser, Debug, Clone)]
pub struct MkdirArgs {
    #[arg(help = "新目录路径，格式: /父目录/新目录名")]
    pub path: String,

    #[arg(short, long, help = "强制继续，如果云端存在同名目录则自动重命名")]
    pub force: bool,
}

pub async fn execute(args: MkdirArgs) -> anyhow::Result<()> {
    let config = crate::config::Config::load()?;
    crate::application::services::mkdir_service::mkdir(&config, &args.path, args.force).await?;
    Ok(())
}
