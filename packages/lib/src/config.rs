use crate::error::{ClientError, ConfigError};
use serde::{Deserialize, Serialize};
use std::borrow::Borrow;
use std::fs;
#[cfg(unix)]
use std::fs::File;
use std::io::{self, Write};
use std::ops::Deref;
use std::path::{Path, PathBuf};

/// 剩余有效期不超过这个时长时，在下一次命令启动时续期 token。
///
/// 这是「何时续期」的唯一阈值：续期接口每次滚动发放 30 天有效的新 token，
/// 留 7 天余量可以容忍几次失败重试。
pub const TOKEN_REFRESH_THRESHOLD_MS: i64 = 7 * 24 * 60 * 60 * 1000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub authorization: String,
    pub account: String,
    pub storage_type: String,
    #[serde(default)]
    pub custom_upload_part_size: i64,
    #[serde(default)]
    pub use_large_thumbnail: bool,
    #[serde(default)]
    pub personal_cloud_host: Option<String>,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub token_expire_time: Option<i64>,
}

const PERSONAL_STORAGE_TYPE: &str = "personal_new";

/// 已通过个人云校验的配置，内部调用可继续使用其只读引用。
#[derive(Clone, Copy)]
pub(crate) struct PersonalConfig<T>(T);

impl<T: Borrow<Config>> PersonalConfig<T> {
    pub(crate) fn new(config: T) -> Result<Self, ClientError> {
        config.borrow().require_personal()?;
        Ok(Self(config))
    }
}

impl PersonalConfig<Config> {
    pub(crate) fn as_ref(&self) -> PersonalConfig<&Config> {
        PersonalConfig(&self.0)
    }

    pub(crate) fn set_cloud_host(&mut self, host: String) {
        self.0.personal_cloud_host = Some(host);
    }

    pub(crate) fn into_inner(self) -> Config {
        self.0
    }
}

impl<T: Borrow<Config>> Deref for PersonalConfig<T> {
    type Target = Config;

    fn deref(&self) -> &Config {
        self.0.borrow()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            authorization: String::new(),
            account: String::new(),
            storage_type: PERSONAL_STORAGE_TYPE.to_owned(),
            custom_upload_part_size: 0,
            use_large_thumbnail: false,
            personal_cloud_host: None,
            refresh_token: None,
            token_expire_time: None,
        }
    }
}

impl Config {
    /// 从指定 TOML 文件读取配置。
    pub fn load_from_path(path: &Path) -> Result<Self, ConfigError> {
        Self::load_from_candidates(&[path.to_owned()]).map(|(config, _)| config)
    }

    /// 返回本次读取的配置和固定来源路径。
    pub fn load_from_candidates(candidates: &[PathBuf]) -> Result<(Self, PathBuf), ConfigError> {
        for path in candidates {
            let path = match canonicalize_config_path(path) {
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

    pub fn require_personal(&self) -> Result<(), ClientError> {
        if self.storage_type == PERSONAL_STORAGE_TYPE {
            Ok(())
        } else {
            Err(ClientError::UnsupportedStorage)
        }
    }

    /// 下一次命令启动时是否需要续期（唯一判断）。
    /// 剩余有效期不超过阈值，或没有过期时间时返回 true。
    pub fn is_token_refresh_due(&self) -> bool {
        self.is_token_refresh_due_at(chrono::Utc::now().timestamp_millis())
    }

    fn is_token_refresh_due_at(&self, now_ms: i64) -> bool {
        match self.token_expire_time {
            Some(expire_time) => expire_time - now_ms <= TOKEN_REFRESH_THRESHOLD_MS,
            None => true,
        }
    }
}

#[doc(hidden)]
pub fn prepare_save_path(path: &Path) -> Result<PathBuf, ConfigError> {
    // 保留已有符号链接的语义，并让相对路径和符号链接共享同一把锁。
    match canonicalize_config_path(path) {
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
    Ok(canonicalize_config_path(parent)?.join(name))
}

fn canonicalize_config_path(path: &Path) -> io::Result<PathBuf> {
    #[cfg(windows)]
    {
        retry_windows_file_operation(|| with_stable_windows_path(path, || fs::canonicalize(path)))
    }
    #[cfg(unix)]
    {
        fs::canonicalize(path)
    }
}

#[cfg(windows)]
fn with_stable_windows_path<T>(
    path: &Path,
    operation: impl FnOnce() -> io::Result<T>,
) -> io::Result<T> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_READ, FILE_SHARE_WRITE,
    };
    // canonicalize 先打开句柄再查询路径。若此时文件被原子替换，Windows
    // 可能返回旧文件在 $Extend\\$Deleted 中的路径，导致后续锁名和写入位置错误。
    // 查询期间以读取句柄拒绝共享删除，固定目录项；零访问权限的元数据句柄
    // 不参与 Windows 的共享访问检查，无法阻止替换。兼容目录与符号链接。
    let _guard = fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)?;
    operation()
}

#[doc(hidden)]
pub fn read_config(path: &Path) -> io::Result<String> {
    #[cfg(windows)]
    {
        retry_windows_file_operation(|| fs::read_to_string(path))
    }
    #[cfg(unix)]
    {
        fs::read_to_string(path)
    }
}

#[cfg(windows)]
fn retry_windows_file_operation<T>(mut operation: impl FnMut() -> io::Result<T>) -> io::Result<T> {
    // Windows 替换已打开的文件时，其他句柄可能短暂得到
    // ERROR_ACCESS_DENIED / ERROR_SHARING_VIOLATION。真实权限错误仍在有界重试后返回。
    for attempt in 0..20 {
        match operation() {
            Err(err) if matches!(err.raw_os_error(), Some(5 | 32)) && attempt < 19 => {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            result => return result,
        }
    }
    unreachable!()
}

#[doc(hidden)]
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
    {
        use std::os::unix::fs::PermissionsExt;
        temp.as_file()
            .set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    temp.write_all(content.as_bytes())?;
    temp.as_file().sync_all()?;
    #[cfg(windows)]
    {
        debug_assert!(overwrite);
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{FILE_ATTRIBUTE_NORMAL, SetFileAttributesW};
        let source: Vec<u16> = temp
            .path()
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect();
        // SAFETY: 路径以 NUL 结尾且在调用期间有效。与 tempfile::persist
        // 一样，在发布配置前清除临时文件属性。
        if unsafe { SetFileAttributesW(source.as_ptr(), FILE_ATTRIBUTE_NORMAL) } == 0 {
            return Err(io::Error::last_os_error().into());
        }
        // std::fs::rename 在 Windows 上提供 FileRenameInfoEx 回退，
        // 可以替换仍有共享读取句柄打开的配置；tempfile::persist 没有该回退。
        // 失败时 temp 仍负责清理原路径，成功时原临时路径已不存在。
        retry_windows_file_operation(|| fs::rename(temp.path(), path))?;
    }
    #[cfg(unix)]
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

    #[cfg(windows)]
    #[test]
    fn canonicalization_blocks_replacement_until_path_is_resolved() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let replacement = dir.path().join("replacement.toml");
        fs::write(&path, "old").unwrap();
        fs::write(&replacement, "new").unwrap();
        let expected = fs::canonicalize(&path).unwrap();
        let resolved = with_stable_windows_path(&path, || {
            let error = fs::rename(&replacement, &path).unwrap_err();
            assert!(matches!(error.raw_os_error(), Some(5 | 32)));
            fs::canonicalize(&path)
        })
        .unwrap();
        assert_eq!(resolved, expected);
        fs::rename(&replacement, &path).unwrap();
        assert_eq!(fs::read_to_string(resolved).unwrap(), "new");
    }

    #[cfg(windows)]
    #[test]
    fn transient_file_errors_are_retried_but_other_errors_are_not() {
        for code in [5, 32] {
            let mut attempts = 0;
            let result = retry_windows_file_operation(|| {
                attempts += 1;
                if attempts < 3 {
                    Err(io::Error::from_raw_os_error(code))
                } else {
                    Ok("complete")
                }
            })
            .unwrap();
            assert_eq!(result, "complete");
            assert_eq!(attempts, 3);
        }
        let mut attempts = 0;
        let error = retry_windows_file_operation::<()>(|| {
            attempts += 1;
            Err(io::Error::from_raw_os_error(123))
        })
        .unwrap_err();
        assert_eq!(attempts, 1);
        assert_eq!(error.raw_os_error(), Some(123));
    }

    #[cfg(windows)]
    #[test]
    fn persistent_file_permission_error_is_returned_after_bounded_retries() {
        let mut attempts = 0;
        let error = retry_windows_file_operation::<()>(|| {
            attempts += 1;
            Err(io::Error::from_raw_os_error(5))
        })
        .unwrap_err();
        assert_eq!(attempts, 20);
        assert_eq!(error.raw_os_error(), Some(5));
    }

    #[cfg(windows)]
    #[test]
    fn atomic_save_replaces_config_while_reader_keeps_old_handle_open() {
        use std::io::Read;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut config = Config {
            account: "old".into(),
            ..Default::default()
        };
        config.save_to(&path).unwrap();
        let mut reader = fs::File::open(&path).unwrap();
        config.account = "new".into();
        config.save_to(&path).unwrap();
        let mut old_content = String::new();
        reader.read_to_string(&mut old_content).unwrap();
        assert_eq!(
            toml::from_str::<Config>(&old_content).unwrap().account,
            "old"
        );
        assert_eq!(
            toml::from_str::<Config>(&fs::read_to_string(&path).unwrap())
                .unwrap()
                .account,
            "new"
        );
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

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
            assert_eq!(
                config.is_token_refresh_due_at(now),
                expected,
                "{remaining:?}"
            );
        }
    }
}
