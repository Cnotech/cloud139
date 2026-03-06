use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use thiserror::Error;

use crate::client::StorageType;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
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

impl Config {
    pub fn config_path() -> PathBuf {
        PathBuf::from("./config/config.json")
    }

    pub fn load() -> Result<Self, ConfigError> {
        let path = Self::config_path();
        if !path.exists() {
            return Err(ConfigError::NotFound);
        }
        let content = fs::read_to_string(&path)?;
        let config: Config = serde_json::from_str(&content)?;
        Ok(config)
    }

    pub fn save(&self) -> Result<(), ConfigError> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
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
