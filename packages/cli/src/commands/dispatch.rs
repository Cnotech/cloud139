use super::{delete, download, list, login, mkdir, sync, upload};
use crate::cli::Commands;
use crate::config::paths::config_paths;
use crate::config::store::refresh_saved_token;
use cloud139::client::auth::refresh::RefreshOptions;
use cloud139::config::Config;
use cloud139::error::{ClientError, ConfigError};
use cloud139::services::{directory, rename::rename};
use std::path::{Path, PathBuf};

/// 校验配置和命令参数，再续期并执行命令。
pub(crate) async fn execute(command: Commands, config_path: Option<&Path>) -> anyhow::Result<()> {
    let paths = config_paths(config_path);
    if let Commands::Login(args) = command {
        return login::execute(args, &paths[0]).await;
    }

    let loaded = Config::load_from_candidates(&paths);
    if let Ok((config, _)) = &loaded {
        config.require_personal()?;
    }

    // Future 在命令参数通过前置检查后才会开始续期。
    let config = prepare_config(loaded);
    match command {
        Commands::Ls(args) => list::execute(args, &config.await?).await,
        Commands::Upload(args) => upload::execute(args, &config.await?).await,
        Commands::Download(args) => download::execute(args, &config.await?).await,
        Commands::Rm(args) => {
            delete::preflight(&args)?;
            delete::execute(args, &config.await?).await
        }
        Commands::Mkdir(args) => mkdir::execute(args, &config.await?).await,
        Commands::Mv(args) => {
            directory::mv(&config.await?, &args.source, &args.target, args.force).await?;
            Ok(())
        }
        Commands::Cp(args) => {
            directory::cp(&config.await?, &args.source, &args.target, args.force).await?;
            Ok(())
        }
        Commands::Rename(args) => {
            rename(&config.await?, &args.source, &args.target, args.force).await?;
            Ok(())
        }
        Commands::Sync(args) => {
            let request = sync::preflight(&args)?;
            sync::execute(args, request, config.await?).await
        }
        Commands::Login(_) => unreachable!("登录已单独处理"),
    }
}

async fn prepare_config(
    loaded: Result<(Config, PathBuf), ConfigError>,
) -> Result<Config, ClientError> {
    let (config, path) = loaded?;
    if !config.is_token_refresh_due() {
        return Ok(config);
    }

    match refresh_saved_token(&path, RefreshOptions::default()).await {
        Err(ClientError::UnsupportedStorage) => return Err(ClientError::UnsupportedStorage),
        Err(err) => crate::warn!("自动续期检查失败: {err}"),
        Ok(()) => {}
    }
    let config = Config::load_from_path(&path)?;
    config.require_personal()?;
    Ok(config)
}
