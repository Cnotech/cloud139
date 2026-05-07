use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use thiserror::Error;

use crate::domain::StorageType;

static CONFIG_OVERRIDE: OnceLock<PathBuf> = OnceLock::new();

pub fn set_config_override(path: PathBuf) {
    let _ = CONFIG_OVERRIDE.set(path);
}

pub fn config_override() -> Option<&'static Path> {
    CONFIG_OVERRIDE.get().map(|p| p.as_path())
}

pub const CONFIG_FILE: &str = "cloud139rc.toml";
pub const GLOBAL_CONFIG_SUBDIR: &str = "cloud139";

pub fn local_config_path() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(CONFIG_FILE)
}

pub fn global_config_path() -> PathBuf {
    if let Some(base) = directories::BaseDirs::new() {
        return base.home_dir().join(".config").join(GLOBAL_CONFIG_SUBDIR).join(CONFIG_FILE);
    }
    PathBuf::from(CONFIG_FILE)
}

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),
    #[error("TOML 解析错误: {0}")]
    TomlDe(#[from] toml::de::Error),
    #[error("TOML 序列化错误: {0}")]
    TomlSer(#[from] toml::ser::Error),
    #[error("配置文件未找到")]
    NotFound,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub authorization: String,
    pub account: String,
    #[serde(default)]
    pub storage_type: String,
    pub cloud_id: Option<String>,
    #[serde(default)]
    pub custom_upload_part_size: i64,
    #[serde(default = "default_true")]
    pub report_real_size: bool,
    #[serde(default)]
    pub use_large_thumbnail: bool,
    #[serde(default)]
    pub personal_cloud_host: Option<String>,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub token_expire_time: Option<i64>,
    #[serde(default)]
    pub root_folder_id: Option<String>,
    #[serde(default)]
    pub user_domain_id: Option<String>,
}

fn default_true() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Self {
            authorization: String::new(),
            account: String::new(),
            storage_type: String::new(),
            cloud_id: None,
            custom_upload_part_size: 0,
            report_real_size: default_true(),
            use_large_thumbnail: false,
            personal_cloud_host: None,
            refresh_token: None,
            token_expire_time: None,
            root_folder_id: None,
            user_domain_id: None,
        }
    }
}

/// 统一解析配置文件路径。
/// 优先级：--config override > 当前目录已存在 > 全局配置目录。
/// 当 `for_write` 为 true 时，若本地不存在，回退到全局（用于写入）。
/// 当 `for_write` 为 false 时，同样遵循此优先级（用于读取单个路径）。
pub fn resolve_config_path(for_write: bool) -> PathBuf {
    if let Some(p) = config_override() {
        return p.to_path_buf();
    }
    let local = local_config_path();
    if local.exists() {
        return local;
    }
    // 当用于写入且本地不存在时，仍返回本地路径（在终端首次登录时写本地）
    if for_write {
        return local;
    }
    global_config_path()
}

/// 返回用于读取的配置文件候选路径列表。
/// 优先级：--config override > [本地, 全局]
pub fn config_load_candidates() -> Vec<PathBuf> {
    if let Some(p) = config_override() {
        return vec![p.to_path_buf()];
    }
    vec![local_config_path(), global_config_path()]
}

impl Config {
    /// 返回默认读取路径（优先当前目录，其次全局）。
    pub fn config_path() -> PathBuf {
        resolve_config_path(false)
    }

    /// save 的默认路径。
    pub fn save_path() -> PathBuf {
        resolve_config_path(true)
    }

    pub fn load() -> Result<Self, ConfigError> {
        let candidates = config_load_candidates();
        for path in &candidates {
            if path.exists() {
                let content = fs::read_to_string(path)?;
                let config: Config = toml::from_str(&content)?;
                return Ok(config);
            }
        }
        Err(ConfigError::NotFound)
    }

    pub fn save(&self) -> Result<(), ConfigError> {
        let path = resolve_config_path(true);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        fs::write(&path, content)?;
        Ok(())
    }

    pub fn storage_type(&self) -> StorageType {
        match self.storage_type.as_str() {
            "family" => StorageType::Family,
            "group" => StorageType::Group,
            _ => StorageType::PersonalNew,
        }
    }

    pub fn is_token_expired(&self) -> bool {
        if let Some(expire_time) = self.token_expire_time {
            let now = chrono::Utc::now().timestamp_millis();
            expire_time - now < 15 * 24 * 60 * 60 * 1000
        } else {
            true
        }
    }
}
