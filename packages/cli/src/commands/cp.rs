use crate::cli::CpArgs;
use cloud139::config::Config;
use cloud139::services::directory::cp;

pub(super) async fn execute(args: CpArgs, config: &Config) -> anyhow::Result<()> {
    cp(config, &args.source, &args.target, args.force).await?;
    Ok(())
}
