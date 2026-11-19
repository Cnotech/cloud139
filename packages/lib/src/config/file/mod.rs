#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(windows)]
use self::windows::{canonicalize, read_to_string};
use crate::config::Config;
use crate::error::ConfigError;
use std::fs;
#[cfg(unix)]
use std::fs::{canonicalize, read_to_string};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

impl Config {
    /// 从指定 TOML 文件读取配置。
    pub fn load_from_path(path: &Path) -> Result<Self, ConfigError> {
        Self::load_from_candidates(&[path.to_owned()]).map(|(config, _)| config)
    }

    /// 返回本次读取的配置和固定来源路径。
    pub fn load_from_candidates(candidates: &[PathBuf]) -> Result<(Self, PathBuf), ConfigError> {
        for path in candidates {
            let path = match canonicalize(path) {
                Ok(path) => path,
                Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
                Err(err) => return Err(err.into()),
            };
            let content = read_config(&path)?;
            return Ok((toml::from_str(&content)?, path));
        }
        Err(ConfigError::NotFound)
    }

    /// 原子替换调用方指定的 TOML 文件。并发的读改写由调用方协调。
    pub fn save_to(&self, path: &Path) -> Result<(), ConfigError> {
        save_atomically(self, &prepare_save_path(path)?, true)
    }
}

pub fn prepare_save_path(path: &Path) -> Result<PathBuf, ConfigError> {
    // 保留已有符号链接的语义，并让相对路径和符号链接共享同一把锁。
    match canonicalize(path) {
        Ok(path) => return Ok(path),
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => return Err(err.into()),
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "配置路径缺少文件名"))?;
    Ok(canonicalize(parent)?.join(name))
}

pub fn read_config(path: &Path) -> io::Result<String> {
    read_to_string(path)
}

pub fn save_atomically(config: &Config, path: &Path, overwrite: bool) -> Result<(), ConfigError> {
    let content = toml::to_string_pretty(config)?;
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "配置路径缺少父目录"))?;
    // NamedTempFile 默认以 0600 创建，凭据从未以更宽权限暴露。
    let mut temp = tempfile::Builder::new()
        .prefix(".cloud139-config-")
        .tempfile_in(parent)?;
    #[cfg(unix)]
    unix::restrict_permissions(temp.as_file())?;
    temp.write_all(content.as_bytes())?;
    temp.as_file().sync_all()?;
    #[cfg(unix)]
    unix::persist(temp, path, parent, overwrite)?;
    #[cfg(windows)]
    windows::persist(temp, path, overwrite)?;
    Ok(())
}
