use clap::Parser;

use cloud139::cli::app::{Cli, Commands};
use cloud139::client::ClientError;
use cloud139::error;
use cloud139::presentation::error::format_error;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    cloud139::utils::logger::init_verbose(&cli.verbose);
    let result = match cli.command {
        Commands::Login(args) => cloud139::commands::login::execute(args.into()).await,
        Commands::Ls(args) => cloud139::commands::list::execute(args).await,
        Commands::Upload(args) => cloud139::commands::upload::execute(args.into()).await,
        Commands::Download(args) => cloud139::commands::download::execute(args.into()).await,
        Commands::Rm(args) => cloud139::commands::delete::execute(args.into()).await,
        Commands::Mkdir(args) => cloud139::commands::mkdir::execute(args.into()).await,
        Commands::Mv(args) => cloud139::commands::mv::execute(args.into()).await,
        Commands::Cp(args) => cloud139::commands::cp::execute(args.into()).await,
        Commands::Rename(args) => cloud139::commands::rename::execute(args.into()).await,
        Commands::Sync(args) => cloud139::commands::sync::execute(args.into()).await,
    };

    if let Err(err) = result {
        if let Some(exit) = err.downcast_ref::<cloud139::commands::sync::CommandExit>() {
            error!("{}", exit);
            std::process::exit(exit.code());
        } else if let Some(client_err) = err.downcast_ref::<ClientError>() {
            error!("{}", format_error(client_err));
            std::process::exit(1);
        } else {
            error!("{}", err);
            std::process::exit(1);
        }
    }

    Ok(())
}
