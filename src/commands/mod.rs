pub mod cp;
pub mod delete;
pub mod download;
pub mod list;
pub mod login;
pub mod mkdir;
pub mod mv;
pub mod rename;
pub mod sync;
pub mod upload;

use crate::cli::app::Commands;
use crate::client::{ClientError, auth};
use crate::config::{Config, ConfigError, config_load_candidates};
use std::{fmt, path::PathBuf};

/// 校验配置和命令参数，再续期并执行命令。
pub async fn execute(command: Commands) -> anyhow::Result<()> {
    if let Commands::Login(args) = command {
        return login::execute(args).await;
    }

    let loaded = Config::load_from_candidates(&config_load_candidates());
    if let Ok((config, _)) = &loaded {
        config.require_personal()?;
    }

    // Future 在命令参数通过前置检查后才会开始续期。
    let config = prepare_config(loaded);
    match command {
        Commands::Ls(args) => list::execute_with_config(args, &config.await?).await,
        Commands::Upload(args) => upload::execute_with_config(args, &config.await?).await,
        Commands::Download(args) => download::execute_with_config(args, &config.await?).await,
        Commands::Rm(args) => {
            delete::preflight(&args)?;
            delete::execute_with_config(args, &config.await?).await
        }
        Commands::Mkdir(args) => mkdir::execute_with_config(args, &config.await?).await,
        Commands::Mv(args) => mv::execute_with_config(args, &config.await?).await,
        Commands::Cp(args) => cp::execute_with_config(args, &config.await?).await,
        Commands::Rename(args) => rename::execute_with_config(args, &config.await?).await,
        Commands::Sync(args) => {
            let endpoints = sync::preflight(&args)?;
            let config = config.await.map_err(sync::config_error)?;
            sync::execute_with_config(args, endpoints, config).await
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

    match auth::refresh_saved_token(&path, auth::RefreshOptions::default()).await {
        Err(ClientError::UnsupportedStorage) => return Err(ClientError::UnsupportedStorage),
        Err(err) => crate::warn!("自动续期检查失败: {err}"),
        Ok(()) => {}
    }
    Config::load_personal()
}

/// 带自定义退出码的命令错误。
#[derive(Debug)]
pub struct CommandExit {
    code: i32,
    message: String,
}

impl CommandExit {
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn code(&self) -> i32 {
        self.code
    }
}

impl fmt::Display for CommandExit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for CommandExit {}
