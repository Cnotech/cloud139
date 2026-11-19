use crate::cli::args::MvArgs;
use cloud139::config::Config;

pub(super) async fn execute(args: MvArgs, config: &Config) -> anyhow::Result<()> {
    cloud139::services::move_service::mv(config, &args.source, &args.target, args.force).await?;
    Ok(())
}
