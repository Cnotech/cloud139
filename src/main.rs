use clap::Parser;

use cloud139::cli::app::{Cli, Commands};
use cloud139::error;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let cli = Cli::parse();
    let result = match cli.command {
        Commands::Ls(args) => cloud139::commands::list::execute(args.into()).await,
        Commands::Upload(args) => cloud139::commands::upload::execute(args.into()).await,
        Commands::Download(args) => cloud139::commands::download::execute(args.into()).await,
    };

    if let Err(err) = result {
        error!("{}", err);
        std::process::exit(1);
    }

    Ok(())
}