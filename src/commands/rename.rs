use clap::Parser;

#[derive(Parser, Debug, Clone)]
pub struct RenameArgs {
    #[arg(help = "源文件或目录路径")]
    pub source: String,

    #[arg(help = "新名称")]
    pub target: String,

    #[arg(short, long, help = "个人盘文件同名时强制继续；目录冲突仍报错")]
    pub force: bool,
}

pub async fn execute(args: RenameArgs) -> anyhow::Result<()> {
    let config = crate::config::Config::load_personal()?;
    crate::services::rename_service::rename(&config, &args.source, &args.target, args.force)
        .await?;
    Ok(())
}
