use crate::cli::CpArgs;
use cloud139::config::Config;

pub(super) async fn execute(args: CpArgs, config: &Config) -> anyhow::Result<()> {
    cloud139::services::copy::cp(config, &args.source, &args.target, args.force).await?;
    Ok(())
}
