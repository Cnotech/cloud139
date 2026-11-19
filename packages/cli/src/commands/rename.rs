use crate::cli::RenameArgs;
use cloud139::config::Config;
use cloud139::services::rename::rename;

pub(super) async fn execute(args: RenameArgs, config: &Config) -> anyhow::Result<()> {
    rename(config, &args.source, &args.target, args.force).await?;
    Ok(())
}
