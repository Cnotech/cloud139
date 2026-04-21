use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use thiserror::Error;

use crate::client::StorageType;

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
    PathBuf::from(format!("./{}", CONFIG_FILE))
}

pub fn global_config_path() -> PathBuf {
    if let Some(base) = directories::BaseDirs::new() {
        return base
            .config_dir()
            .join(GLOBAL_CONFIG_SUBDIR)
            .join(CONFIG_FILE);
    }
    PathBuf::from(CONFIG_FILE)
}

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("TOML error: {0}")]
    TomlDe(#[from] toml::de::Error),
    #[error("TOML serialize error: {0}")]
    TomlSer(#[from] toml::ser::Error),
    #[error("Config not found")]
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

impl Config {
    /// 返回默认读取路径（优先当前目录，其次全局）。
    /// 注意：load/save 会综合 override、当前目录、全局目录自行决定路径。
    pub fn config_path() -> PathBuf {
        if let Some(p) = config_override() {
            return p.to_path_buf();
        }
        let local = local_config_path();
        if local.exists() {
            return local;
        }
        global_config_path()
    }

    /// save 的默认路径：override > 全局。
    pub fn save_path() -> PathBuf {
        if let Some(p) = config_override() {
            return p.to_path_buf();
        }
        global_config_path()
    }

    pub fn load() -> Result<Self, ConfigError> {
        let candidates: Vec<PathBuf> = if let Some(p) = config_override() {
            vec![p.to_path_buf()]
        } else {
            vec![local_config_path(), global_config_path()]
        };
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
        // override > 当前目录已存在 > 全局
        let path = if let Some(p) = config_override() {
            p.to_path_buf()
        } else {
            let local = local_config_path();
            if local.exists() {
                local
            } else {
                global_config_path()
            }
        };
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
