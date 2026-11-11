//! 跨进程配置锁，不创建磁盘锁文件。

#[cfg(unix)]
pub(super) use unix::ConfigLock;
#[cfg(windows)]
pub(super) use windows::ConfigLock;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, ConfigError};
    use std::fs;

    #[test]
    fn lock_holder_child() {
        let Some(path) = std::env::var_os("CLOUD139_TEST_LOCK_PATH") else {
            return;
        };
        let path = std::path::PathBuf::from(path);
        let _lock = ConfigLock::acquire(&path).unwrap();
        fs::write(path.with_extension("ready"), "ready").unwrap();
        std::thread::sleep(std::time::Duration::from_secs(30));
    }

    #[test]
    fn lock_is_cross_process_and_released_after_crash() {
        use std::time::{Duration, Instant};
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        Config::default().save_to(&path).unwrap();
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "config::lock::tests::lock_holder_child"])
            .env("CLOUD139_TEST_LOCK_PATH", &path)
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let start = Instant::now();
        while !path.with_extension("ready").exists() && start.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(10));
        }
        let ready = path.with_extension("ready").exists();
        let result = ConfigLock::acquire(&path);
        child.kill().unwrap();
        child.wait().unwrap();
        assert!(ready, "child did not acquire the config lock");
        assert!(matches!(result, Err(ConfigError::Busy)));
        assert!(ConfigLock::acquire(&path).is_ok());
        assert!(!path.with_extension("toml.lock").exists());
    }

    #[test]
    fn locks_are_per_config_and_release_on_drop() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("first.toml");
        let second = dir.path().join("second.toml");
        Config::default().save_to(&first).unwrap();
        Config::default().save_to(&second).unwrap();
        let first_lock = ConfigLock::acquire(&first).unwrap();
        assert!(matches!(
            ConfigLock::acquire(&first),
            Err(ConfigError::Busy)
        ));
        let second_lock = ConfigLock::acquire(&second).unwrap();
        drop(first_lock);
        assert!(ConfigLock::acquire(&first).is_ok());
        drop(second_lock);
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
    }

    #[cfg(unix)]
    #[test]
    fn symlink_and_target_share_lock() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let alias = dir.path().join("alias.toml");
        Config::default().save_to(&path).unwrap();
        std::os::unix::fs::symlink(&path, &alias).unwrap();
        let _lock = ConfigLock::acquire(&path).unwrap();
        assert!(matches!(
            ConfigLock::acquire(&alias),
            Err(ConfigError::Busy)
        ));
    }
}

#[cfg(unix)]
mod unix {
    use crate::config::ConfigError;
    use fs2::FileExt;
    use std::fs::{self, File, OpenOptions};
    use std::io::{self, Read, Seek};
    use std::os::unix::fs::MetadataExt;
    use std::path::Path;

    pub(crate) struct ConfigLock {
        file: File,
    }

    impl ConfigLock {
        pub(crate) fn acquire(path: &Path) -> Result<Self, ConfigError> {
            // 另一个写入者可能在 open 与 flock 之间原子替换配置。
            // 有界重试，避免一直追逐被外部程序反复替换的文件。
            for _ in 0..16 {
                let file = OpenOptions::new().read(true).write(true).open(path)?;
                if let Some(lock) = Self::acquire_open_file(path, file)? {
                    return Ok(lock);
                }
            }
            Err(ConfigError::Busy)
        }

        fn acquire_open_file(path: &Path, file: File) -> Result<Option<Self>, ConfigError> {
            if !file.metadata()?.is_file() {
                return Err(
                    io::Error::new(io::ErrorKind::InvalidInput, "配置路径不是普通文件").into(),
                );
            }
            let result = FileExt::try_lock_exclusive(&file);
            let lock = Self { file };
            // 即使旧文件仍忙，也应重新打开当前路径，而不是使用过期的 inode。
            if !lock.is_current(path)? {
                return Ok(None);
            }
            match result {
                Ok(()) => Ok(Some(lock)),
                Err(err) if err.raw_os_error() == fs2::lock_contended_error().raw_os_error() => {
                    Err(ConfigError::Busy)
                }
                Err(err) => Err(err.into()),
            }
        }

        fn is_current(&self, path: &Path) -> Result<bool, ConfigError> {
            let current = fs::metadata(path)?;
            let locked = self.file.metadata()?;
            Ok(current.dev() == locked.dev() && current.ino() == locked.ino())
        }

        pub(crate) fn read_to_string(&mut self, path: &Path) -> Result<String, ConfigError> {
            if !self.is_current(path)? {
                return Err(ConfigError::Changed);
            }
            // 通过持锁句柄读取，也兼容会强制执行文件锁的文件系统。
            self.file.rewind()?;
            let mut content = String::new();
            self.file.read_to_string(&mut content)?;
            if !self.is_current(path)? {
                return Err(ConfigError::Changed);
            }
            Ok(content)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn rejects_stale_open_file_after_atomic_replacement() {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("config.toml");
            fs::write(&path, "old").unwrap();
            let stale = File::open(&path).unwrap();
            let old_lock = ConfigLock::acquire(&path).unwrap();
            let replacement = dir.path().join("replacement");
            fs::write(&replacement, "new").unwrap();
            fs::rename(replacement, &path).unwrap();
            // 即使旧 inode 仍被锁住，也不能将它误认为当前配置。
            assert!(
                ConfigLock::acquire_open_file(&path, stale)
                    .unwrap()
                    .is_none()
            );
            let mut current = ConfigLock::acquire(&path).unwrap();
            assert_eq!(current.read_to_string(&path).unwrap(), "new");
            drop(old_lock);
            assert!(matches!(ConfigLock::acquire(&path), Err(ConfigError::Busy)));
        }

        #[test]
        fn rejects_replaced_file_even_when_contents_are_identical() {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("config.toml");
            fs::write(&path, "same").unwrap();
            let mut lock = ConfigLock::acquire(&path).unwrap();
            let replacement = dir.path().join("replacement");
            fs::write(&replacement, "same").unwrap();
            fs::rename(replacement, &path).unwrap();
            assert!(matches!(
                lock.read_to_string(&path),
                Err(ConfigError::Changed)
            ));
        }
    }
}

#[cfg(windows)]
mod windows {
    use crate::config::ConfigError;
    use sha2::{Digest, Sha256};
    use std::fs;
    use std::io;
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use std::path::Path;
    use std::sync::mpsc::{self, Sender};
    use std::thread::{self, JoinHandle};
    use windows_sys::Win32::Foundation::{WAIT_ABANDONED, WAIT_OBJECT_0, WAIT_TIMEOUT};
    use windows_sys::Win32::System::Threading::{CreateMutexW, ReleaseMutex, WaitForSingleObject};

    pub(crate) struct ConfigLock {
        release: Option<Sender<()>>,
        worker: Option<JoinHandle<()>>,
    }

    fn mutex_name(path: &Path) -> Result<String, ConfigError> {
        let path = super::super::prepare_save_path(path)?;
        // canonicalize 解析符号链接、相对路径及已有文件的路径别名。
        // 大小写归一避免首次创建前后因拼写大小写不同而使用不同互斥量。
        let normalized = path.to_string_lossy().to_lowercase();
        Ok(format!(
            "Global\\cloud139-config-{}",
            hex::encode(Sha256::digest(normalized.as_bytes()))
        ))
    }

    impl ConfigLock {
        pub(crate) fn acquire(path: &Path) -> Result<Self, ConfigError> {
            let name = mutex_name(path)?;
            let (ready_tx, ready_rx) = mpsc::sync_channel(1);
            let (release_tx, release_rx) = mpsc::channel();
            // Win32 互斥量属于线程。专用线程持锁，保证跨 await 或跨线程
            // 丢弃 ConfigLock 时，仍由获得互斥量的原线程释放。
            let worker = thread::Builder::new()
                .name("config-lock".into())
                .spawn(move || match MutexOwner::acquire(&name) {
                    Ok(_owner) => {
                        if ready_tx.send(Ok(())).is_ok() {
                            let _ = release_rx.recv();
                        }
                    }
                    Err(err) => {
                        let _ = ready_tx.send(Err(err));
                    }
                })?;
            match ready_rx.recv() {
                Ok(Ok(())) => Ok(Self {
                    release: Some(release_tx),
                    worker: Some(worker),
                }),
                result => {
                    drop(release_tx);
                    let _ = worker.join();
                    match result {
                        Ok(Err(err)) => Err(err),
                        _ => Err(io::Error::other("配置锁线程意外退出").into()),
                    }
                }
            }
        }

        pub(crate) fn read_to_string(&mut self, path: &Path) -> Result<String, ConfigError> {
            Ok(fs::read_to_string(path)?)
        }
    }

    impl Drop for ConfigLock {
        fn drop(&mut self) {
            // 关闭通道通知原线程释放，并等待句柄关闭后再返回。
            self.release.take();
            if let Some(worker) = self.worker.take() {
                let _ = worker.join();
            }
        }
    }

    struct MutexOwner(OwnedHandle);

    impl MutexOwner {
        fn acquire(name: &str) -> Result<Self, ConfigError> {
            let name: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
            // SAFETY: 名称以 NUL 结尾且在调用期间有效；不继承句柄。
            let raw = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
            if raw.is_null() {
                return Err(io::Error::last_os_error().into());
            }
            // SAFETY: CreateMutexW 返回的有效句柄在这里转移唯一所有权。
            let handle = unsafe { OwnedHandle::from_raw_handle(raw) };
            // SAFETY: 句柄有效；零超时保持原有的非阻塞行为。
            match unsafe { WaitForSingleObject(handle.as_raw_handle(), 0) } {
                WAIT_OBJECT_0 | WAIT_ABANDONED => Ok(Self(handle)),
                WAIT_TIMEOUT => Err(ConfigError::Busy),
                _ => Err(io::Error::last_os_error().into()),
            }
        }
    }

    impl Drop for MutexOwner {
        fn drop(&mut self) {
            // SAFETY: 此对象始终在获取互斥量的专用线程内创建并销毁。
            unsafe {
                ReleaseMutex(self.0.as_raw_handle());
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn path_aliases_share_mutex_and_different_paths_do_not() {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("Config.toml");
            fs::write(&path, "config").unwrap();
            let name = mutex_name(&path).unwrap();
            assert_eq!(
                name,
                mutex_name(&dir.path().join(".\\config.toml")).unwrap()
            );
            assert_ne!(name, mutex_name(&dir.path().join("other.toml")).unwrap());
            assert!(name.starts_with("Global\\cloud139-config-"));
            assert!(name.len() < 260);
        }

        #[test]
        fn mutex_can_be_dropped_on_another_thread() {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("config.toml");
            let lock = ConfigLock::acquire(&path).unwrap();
            assert!(matches!(ConfigLock::acquire(&path), Err(ConfigError::Busy)));
            thread::spawn(move || drop(lock)).join().unwrap();
            assert!(ConfigLock::acquire(&path).is_ok());
            assert!(fs::read_dir(dir.path()).unwrap().next().is_none());
        }
    }
}
