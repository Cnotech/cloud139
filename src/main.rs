use clap::Parser;

use cloud139::cli::app::{Cli, Commands};
use cloud139::client::ClientError;
use cloud139::commands::{self, CommandExit};
use cloud139::config;
use cloud139::error;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    if matches!(&cli.command, Commands::Sync(args) if args.json) {
        cloud139::utils::logger::route_status_to_stderr();
    }
    if let Some(path) = cli.config.clone() {
        config::set_config_override(path);
    }
    cloud139::utils::logger::init_verbose(&cli.verbose);

    if let Err(err) = commands::execute(cli.command).await {
        if let Some(exit) = err.downcast_ref::<CommandExit>() {
            error!("{}", exit);
            std::process::exit(exit.code());
        } else if let Some(client_err) = err.downcast_ref::<ClientError>() {
            error!("{}", client_err);
            std::process::exit(1);
        } else {
            error!("{}", err);
            std::process::exit(1);
        }
    }

    Ok(())
}
