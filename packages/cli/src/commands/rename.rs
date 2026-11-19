use crate::cli::RenameArgs;
use cloud139::config::Config;

pub(super) async fn execute(args: RenameArgs, config: &Config) -> anyhow::Result<()> {
    cloud139::services::rename_service::rename(config, &args.source, &args.target, args.force)
        .await?;
    Ok(())
}
