#![warn(unreachable_pub)]

mod cli;
mod commands;
mod config;
mod error;
mod logger;
mod presentation;

#[cfg(test)]
mod tests;

use clap::Parser;
use cli::{Cli, Commands};
use error::CommandExit;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    if matches!(&cli.command, Commands::Sync(args) if args.json) {
        logger::route_status_to_stderr();
    }
    logger::init_verbose(&cli.verbose);
    logger::init_core_logger();

    if let Err(err) = commands::execute(cli.command, cli.config.as_deref()).await {
        let code = err
            .downcast_ref::<CommandExit>()
            .map_or(1, CommandExit::code);
        error!("{err}");
        std::process::exit(code);
    }

    Ok(())
}
