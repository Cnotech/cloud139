use crate::cli::MvArgs;
use cloud139::config::Config;
use cloud139::services::directory::mv;

pub(super) async fn execute(args: MvArgs, config: &Config) -> anyhow::Result<()> {
    mv(config, &args.source, &args.target, args.force).await?;
    Ok(())
}
