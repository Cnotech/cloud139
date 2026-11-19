use crate::cli::UploadArgs;
use cloud139::config::Config;
use std::io::IsTerminal;

pub(super) async fn execute(args: UploadArgs, config: &Config) -> anyhow::Result<()> {
    let progress = std::io::stderr().is_terminal();
    cloud139::services::upload_service::upload_path(
        config,
        &args.local_path,
        &args.remote_path,
        args.force,
        |name, size| {
            if progress {
                crate::presentation::progress::make_progress(name, size)
            } else {
                None
            }
        },
    )
    .await
}
