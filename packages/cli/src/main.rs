#![warn(unreachable_pub)]

mod cli;
mod commands;
mod config;
mod error;
mod log_file;
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
    if let Some(path) = &cli.log_file {
        let file = log_file::create(path, &cli)?;
        logger::init_file(file);
    }
    logger::init_verbose(&cli.verbose);
    logger::init_core_logger();

    if let Err(err) = commands::dispatch::execute(cli.command, cli.config.as_deref()).await {
        let code = err
            .downcast_ref::<CommandExit>()
            .map_or(1, CommandExit::code);
        debug!("命令执行失败 exit_code={code}");
        error!("{err}");
        logger::flush_file();
        std::process::exit(code);
    }

    logger::flush_file();
    Ok(())
}
