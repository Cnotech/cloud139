use clap::Parser;
use cloud139::config::Config;

#[derive(Parser, Debug, Clone)]
pub struct RenameArgs {
    #[arg(help = "源文件或目录路径")]
    pub source: String,

    #[arg(help = "新名称")]
    pub target: String,

    #[arg(
        short,
        long,
        help = "新名称已被其他文件使用时，仍提交重命名。涉及目录的同名冲突会报错"
    )]
    pub force: bool,
}

pub(super) async fn execute(args: RenameArgs, config: &Config) -> anyhow::Result<()> {
    cloud139::services::rename_service::rename(config, &args.source, &args.target, args.force)
        .await?;
    Ok(())
}
