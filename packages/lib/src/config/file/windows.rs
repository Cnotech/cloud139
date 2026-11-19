use std::fs;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;
use windows_sys::Win32::Storage::FileSystem::{
    FILE_ATTRIBUTE_NORMAL, FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_READ, FILE_SHARE_WRITE,
    SetFileAttributesW,
};

pub(super) fn canonicalize(path: &Path) -> io::Result<PathBuf> {
    retry_windows_file_operation(|| with_stable_windows_path(path, || fs::canonicalize(path)))
}

fn with_stable_windows_path<T>(
    path: &Path,
    operation: impl FnOnce() -> io::Result<T>,
) -> io::Result<T> {
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

pub(super) fn read_to_string(path: &Path) -> io::Result<String> {
    retry_windows_file_operation(|| fs::read_to_string(path))
}

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

pub(super) fn persist(temp: NamedTempFile, path: &Path, overwrite: bool) -> io::Result<()> {
    debug_assert!(overwrite);
    let source: Vec<u16> = temp
        .path()
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    // SAFETY: 路径以 NUL 结尾且在调用期间有效。与 tempfile::persist
    // 一样，在发布配置前清除临时文件属性。
    if unsafe { SetFileAttributesW(source.as_ptr(), FILE_ATTRIBUTE_NORMAL) } == 0 {
        return Err(io::Error::last_os_error());
    }
    // std::fs::rename 在 Windows 上提供 FileRenameInfoEx 回退，
    // 可以替换仍有共享读取句柄打开的配置；tempfile::persist 没有该回退。
    // 失败时 temp 仍负责清理原路径，成功时原临时路径已不存在。
    retry_windows_file_operation(|| fs::rename(temp.path(), path))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

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
}
