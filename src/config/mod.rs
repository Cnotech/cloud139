use serde::{Deserialize, Serialize};
use std::fs;
#[cfg(unix)]
use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use thiserror::Error;

use crate::domain::StorageType;

mod lock;
use lock::ConfigLock;

static CONFIG_OVERRIDE: OnceLock<PathBuf> = OnceLock::new();

pub fn set_config_override(path: PathBuf) {
    let _ = CONFIG_OVERRIDE.set(path);
}

pub fn config_override() -> Option<&'static Path> {
    CONFIG_OVERRIDE.get().map(|p| p.as_path())
}

pub const CONFIG_FILE: &str = "cloud139rc.toml";
pub const GLOBAL_CONFIG_SUBDIR: &str = "cloud139";

/// 剩余有效期不超过这个时长时，在下一次命令启动时续期 token。
///
/// 这是「何时续期」的唯一阈值：续期接口每次滚动发放 30 天有效的新 token，
/// 留 7 天余量可以容忍几次失败重试。
pub const TOKEN_REFRESH_THRESHOLD_MS: i64 = 7 * 24 * 60 * 60 * 1000;

pub fn local_config_path() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(CONFIG_FILE)
}

pub fn global_config_path() -> PathBuf {
    if let Some(base) = directories::BaseDirs::new() {
        return base
            .home_dir()
            .join(".config")
            .join(GLOBAL_CONFIG_SUBDIR)
            .join(CONFIG_FILE);
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
    #[error("配置正在被另一个进程更新")]
    Busy,
    #[error("配置已被其他操作修改，放弃写回续期结果")]
    Changed,
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
        Self::load_with_path().map(|(config, _)| config)
    }

    /// 一起返回配置及本次读取的固定来源路径，不能在网络请求后重新搜索路径。
    pub fn load_with_path() -> Result<(Self, PathBuf), ConfigError> {
        Self::load_from_candidates(&config_load_candidates())
    }

    pub(crate) fn load_from_candidates(
        candidates: &[PathBuf],
    ) -> Result<(Self, PathBuf), ConfigError> {
        for path in candidates {
            let path = match fs::canonicalize(path) {
                Ok(path) => path,
                Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
                Err(err) => return Err(err.into()),
            };
            let content = fs::read_to_string(&path)?;
            return Ok((toml::from_str(&content)?, path));
        }
        Err(ConfigError::NotFound)
    }

    pub fn save(&self) -> Result<(), ConfigError> {
        self.save_to(&resolve_config_path(true))
    }

    /// 同目录临时文件完整写入并同步后原子替换。写入期间不会暴露空文件。
    /// 使用系统锁协调写入，不创建锁文件；锁忙时返回错误而非无限等待。
    pub fn save_to(&self, path: &Path) -> Result<(), ConfigError> {
        let path = prepare_save_path(path)?;
        #[cfg(windows)]
        {
            let _lock = ConfigLock::acquire(&path)?;
            save_atomically(self, &path, true)
        }
        #[cfg(unix)]
        {
            for _ in 0..16 {
                match ConfigLock::acquire(&path) {
                    Ok(_lock) => return save_atomically(self, &path, true),
                    // 没有可供加锁的文件时，直接原子创建完整配置。
                    // 另一个进程抢先创建成功则重新打开并加锁。
                    Err(ConfigError::Io(err)) if err.kind() == io::ErrorKind::NotFound => {
                        match save_atomically(self, &path, false) {
                            Err(ConfigError::Io(err))
                                if err.kind() == io::ErrorKind::AlreadyExists => {}
                            result => return result,
                        }
                    }
                    Err(err) => return Err(err),
                }
            }
            Err(ConfigError::Busy)
        }
    }

    /// 网络请求前为缓存更新固定来源；内存配置不属于当前文件时不落盘。
    pub(crate) fn cache_path(&self) -> Option<PathBuf> {
        let (current, path) = Self::load_with_path().ok()?;
        (self.same_account(&current)
            && self.authorization == current.authorization
            && self.refresh_token == current.refresh_token)
            .then_some(path)
    }

    /// 缓存更新只修改指定字段，不能把网络请求前的旧 token 全量写回。
    pub(crate) fn update_cache(
        &self,
        path: &Path,
        change: impl FnOnce(&mut Config),
    ) -> Result<(), ConfigError> {
        let update = ConfigUpdate::load(path)?;
        if !self.same_account(&update.config) {
            return Err(ConfigError::Changed);
        }
        let mut current = update.config.clone();
        change(&mut current);
        update.save(&current)
    }

    fn same_account(&self, other: &Config) -> bool {
        self.account == other.account
            && self.storage_type == other.storage_type
            && self.cloud_id == other.cloud_id
    }

    pub fn storage_type(&self) -> StorageType {
        match self.storage_type.as_str() {
            "family" => StorageType::Family,
            "group" => StorageType::Group,
            _ => StorageType::PersonalNew,
        }
    }

    /// 下一次命令启动时是否需要续期（唯一判断）。
    /// 剩余有效期不超过阈值，或没有过期时间时返回 true。
    pub fn needs_refresh(&self) -> bool {
        self.needs_refresh_at(chrono::Utc::now().timestamp_millis())
    }

    fn needs_refresh_at(&self, now_ms: i64) -> bool {
        match self.token_expire_time {
            Some(expire_time) => expire_time - now_ms <= TOKEN_REFRESH_THRESHOLD_MS,
            None => true,
        }
    }
}

/// 锁覆盖读取、判断、网络续期和写回；Unix 校验被锁文件仍为当前配置，
/// Windows 使用与规范化配置路径对应的命名互斥量。
pub(crate) struct ConfigUpdate {
    pub config: Config,
    path: PathBuf,
    original: String,
    lock: ConfigLock,
}

impl ConfigUpdate {
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let mut lock = ConfigLock::acquire(path)?;
        let original = lock.read_to_string(path)?;
        let config = toml::from_str(&original)?;
        Ok(Self {
            config,
            path: path.to_path_buf(),
            original,
            lock,
        })
    }

    pub fn save(mut self, config: &Config) -> Result<(), ConfigError> {
        // 也保护不遵循系统锁的编辑器或旧版本程序在网络等待期间写入的内容。
        if self.lock.read_to_string(&self.path)? != self.original {
            return Err(ConfigError::Changed);
        }
        save_atomically(config, &self.path, true)
    }
}

fn prepare_save_path(path: &Path) -> Result<PathBuf, ConfigError> {
    // 保留已有符号链接的语义，并让相对路径和符号链接共享同一把锁。
    match fs::canonicalize(path) {
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
    Ok(fs::canonicalize(parent)?.join(name))
}

fn save_atomically(config: &Config, path: &Path, overwrite: bool) -> Result<(), ConfigError> {
    let content = toml::to_string_pretty(config)?;
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "配置路径缺少父目录"))?;
    // NamedTempFile 默认以 0600 创建，凭据从未以更宽权限暴露。
    let mut temp = tempfile::Builder::new()
        .prefix(".cloud139-config-")
        .tempfile_in(parent)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        temp.as_file()
            .set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    temp.write_all(content.as_bytes())?;
    temp.as_file().sync_all()?;
    if overwrite {
        temp.persist(path).map_err(|err| err.error)?;
    } else {
        temp.persist_noclobber(path).map_err(|err| err.error)?;
    }
    // Unix 上同步目录，保证重命名的目录项也被持久化。
    #[cfg(unix)]
    File::open(parent)?.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod refresh_tests {
    use super::*;

    #[test]
    fn refresh_threshold_includes_exactly_seven_days() {
        let now = 1_800_000_000_000;
        let seven_days = 7 * 86_400_000;
        for (remaining, expected) in [
            (None, true),
            (Some(-1), true),
            (Some(0), true),
            (Some(seven_days - 1), true),
            (Some(seven_days), true),
            (Some(seven_days + 1), false),
            (Some(30 * 86_400_000), false),
        ] {
            let config = Config {
                token_expire_time: remaining.map(|ms| now + ms),
                ..Default::default()
            };
            assert_eq!(config.needs_refresh_at(now), expected, "{remaining:?}");
        }
    }
}
