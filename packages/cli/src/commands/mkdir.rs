use crate::cli::MkdirArgs;
use cloud139::config::Config;
use cloud139::services::mkdir_service::{MkdirMode, mkdir};

pub(super) async fn execute(args: MkdirArgs, config: &Config) -> anyhow::Result<()> {
    let mode = if args.parents {
        MkdirMode::Parents
    } else {
        MkdirMode::Single
    };
    mkdir(config, &args.path, mode).await?;
    Ok(())
}
