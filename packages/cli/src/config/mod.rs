use crate::error::ConfigWriteError;
#[cfg(windows)]
use cloud139::config::read_config;
use cloud139::{client::auth, error::ClientError};
use cloud139::{
    config::{Config, prepare_save_path, save_atomically},
    error::ConfigError,
};
use std::io;
use std::path::{Path, PathBuf};

mod lock;
use lock::ConfigLock;

const CONFIG_FILE: &str = "cloud139rc.toml";
const GLOBAL_CONFIG_SUBDIR: &str = "cloud139";

fn local_config_path() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(CONFIG_FILE)
}

fn global_config_path() -> PathBuf {
    if let Some(base) = directories::BaseDirs::new() {
        return base
            .home_dir()
            .join(".config")
            .join(GLOBAL_CONFIG_SUBDIR)
            .join(CONFIG_FILE);
    }
    PathBuf::from(CONFIG_FILE)
}

pub(super) fn config_paths(override_path: Option<&Path>) -> Vec<PathBuf> {
    match override_path {
        Some(path) => vec![path.to_owned()],
        None => vec![local_config_path(), global_config_path()],
    }
}

pub(super) fn save_config(config: &Config, path: &Path) -> Result<(), ConfigWriteError> {
    let path = prepare_save_path(path)?;
    #[cfg(windows)]
    {
        let _lock = ConfigLock::acquire(&path)?;
        Ok(save_atomically(config, &path, true)?)
    }
    #[cfg(unix)]
    {
        for _ in 0..16 {
            match ConfigLock::acquire(&path) {
                Ok(_lock) => return Ok(save_atomically(config, &path, true)?),
                // 没有可供加锁的文件时，直接原子创建完整配置。
                // 另一个进程抢先创建成功则重新打开并加锁。
                Err(ConfigWriteError::File(ConfigError::Io(err)))
                    if err.kind() == io::ErrorKind::NotFound =>
                {
                    match save_atomically(config, &path, false) {
                        Err(ConfigError::Io(err)) if err.kind() == io::ErrorKind::AlreadyExists => {
                        }
                        result => return result.map_err(Into::into),
                    }
                }
                Err(err) => return Err(err),
            }
        }
        Err(ConfigWriteError::Busy)
    }
}

/// 锁覆盖读取、判断、网络续期和写回；Unix 校验被锁文件仍为当前配置，
/// Windows 使用与规范化配置路径对应的命名互斥量。
struct ConfigUpdate {
    config: Config,
    path: PathBuf,
    original: String,
    lock: ConfigLock,
}

impl ConfigUpdate {
    fn load(path: &Path) -> Result<Self, ConfigWriteError> {
        let mut lock = ConfigLock::acquire(path)?;
        let original = lock.read_to_string(path)?;
        let config = toml::from_str(&original).map_err(ConfigError::from)?;
        Ok(Self {
            config,
            path: path.to_path_buf(),
            original,
            lock,
        })
    }

    fn save(mut self, config: &Config) -> Result<(), ConfigWriteError> {
        // 也保护不遵循系统锁的编辑器或旧版本程序在网络等待期间写入的内容。
        if self.lock.read_to_string(&self.path)? != self.original {
            return Err(ConfigWriteError::Changed);
        }
        Ok(save_atomically(config, &self.path, true)?)
    }
}

pub(super) async fn refresh_saved_token(
    path: &Path,
    options: auth::RefreshOptions<'_>,
) -> Result<(), ClientError> {
    // 锁内重新读取并判断，使同一份配置的续期请求串行执行。
    let update = match ConfigUpdate::load(path) {
        Ok(update) => update,
        Err(err) => {
            crate::warn!("自动续期跳过（继续使用现有 Token）: {err}");
            return Ok(());
        }
    };
    match auth::refresh_token_if_due(&update.config, options).await {
        Ok(Some(config)) => {
            update
                .save(&config)
                .map_err(|err| ClientError::Other(format!("保存配置失败: {err}")))?;
            crate::debug!("已将续期后的 Token 写回配置 path={}", path.display());
        }
        Ok(None) => {}
        Err(ClientError::UnsupportedStorage) => return Err(ClientError::UnsupportedStorage),
        Err(err) => crate::warn!("自动续期失败（继续使用现有 Token）: {err}"),
    }
    Ok(())
}
