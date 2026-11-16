use clap::Parser;

use cloud139::cli::app::{Cli, Commands};
use cloud139::client::ClientError;
use cloud139::error;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    if let Some(path) = cli.config.clone() {
        cloud139::config::set_config_override(path);
    }
    cloud139::utils::logger::init_verbose(&cli.verbose);

    // mirror --json：stdout 只留给汇总 JSON，日志（含续期/重试告警）一律走 stderr。
    if let Commands::Mirror(args) = &cli.command {
        cloud139::utils::logger::set_json_mode(args.json);
    }

    // 除登录外的命令，启动时按需续期。139 的 refresh 接口每次滚动发放 30 天有效的新 token，
    // 所以只要 CLI 每天被调用，就不需要额外的定时任务来维持登录态。
    if !matches!(&cli.command, Commands::Login(_))
        && let Err(err) = cloud139::client::auth::maybe_refresh_config().await
    {
        cloud139::warn!("自动续期检查失败: {}", err);
    }

    let result = match cli.command {
        Commands::Login(args) => cloud139::commands::login::execute(args).await,
        Commands::Ls(args) => cloud139::commands::list::execute(args).await,
        Commands::Upload(args) => cloud139::commands::upload::execute(args).await,
        Commands::Download(args) => cloud139::commands::download::execute(args).await,
        Commands::Rm(args) => cloud139::commands::delete::execute(args).await,
        Commands::Mkdir(args) => cloud139::commands::mkdir::execute(args).await,
        Commands::Mv(args) => cloud139::commands::mv::execute(args).await,
        Commands::Cp(args) => cloud139::commands::cp::execute(args).await,
        Commands::Rename(args) => cloud139::commands::rename::execute(args).await,
        Commands::Sync(args) => cloud139::commands::sync::execute(args).await,
        Commands::Mirror(args) => cloud139::commands::mirror::execute(args).await,
    };

    if let Err(err) = result {
        if let Some(exit) = err.downcast_ref::<cloud139::commands::CommandExit>() {
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
