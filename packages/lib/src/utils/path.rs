use crate::error::{ClientError, invalid};
use std::path::{Path, PathBuf};

/// 根据远程路径和可选的本地路径，解析出最终的本地保存路径。
pub fn resolve_local_path(remote_path: &str, local_path: &Option<String>) -> String {
    match local_path {
        Some(path) if !path.is_empty() => {
            let ends_with_slash = path.ends_with('/');
            let path = path.trim_end_matches('/');
            let path_obj = Path::new(path);
            if path_obj.is_dir() || ends_with_slash {
                let parts: Vec<&str> = remote_path.trim_start_matches('/').rsplit('/').collect();
                let file_name = parts.first().copied().unwrap_or(remote_path);
                if file_name.is_empty() || file_name == remote_path {
                    format!("{}/download", path)
                } else {
                    format!("{}/{}", path, file_name)
                }
            } else {
                path.to_string()
            }
        }
        _ => {
            let parts: Vec<&str> = remote_path.trim_start_matches('/').rsplit('/').collect();
            let file_name = parts.first().copied().unwrap_or(remote_path);
            if file_name.is_empty() || file_name == remote_path {
                "download".to_string()
            } else {
                file_name.to_string()
            }
        }
    }
}

/// 检查路径中每个已存在的部分（含调用方指定的父目录），发现符号链接即拒绝。
pub fn safe_local(path: &Path) -> Result<(), ClientError> {
    let absolute = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()?.join(path)
    };
    #[cfg(target_os = "macos")]
    let absolute = {
        let mut resolved = absolute.clone();
        for alias in ["/var", "/tmp", "/etc"] {
            if let Ok(rest) = absolute.strip_prefix(alias) {
                resolved = std::fs::canonicalize(alias)?.join(rest);
                break;
            }
        }
        resolved
    };
    let mut current = PathBuf::new();
    for component in absolute.components() {
        if component == std::path::Component::ParentDir {
            return Err(invalid("本地路径不允许 .."));
        }
        current.push(component);
        match std::fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(invalid(format!("拒绝符号链接: {}", current.display())));
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

pub(crate) fn local_parent_directory(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

pub(crate) fn check_destination_collisions<'a>(
    destination_root: &Path,
    paths: impl IntoIterator<Item = (&'a Path, bool)>,
) -> anyhow::Result<()> {
    let mut paths = paths.into_iter().peekable();
    if paths.peek().is_none() {
        return Ok(());
    }
    let mut parent = destination_root;
    loop {
        match std::fs::metadata(parent) {
            Ok(metadata) if metadata.is_dir() => break,
            Ok(_) => return Err(invalid("本地文件与目录冲突").into()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                parent = local_parent_directory(parent)
            }
            Err(error) => return Err(error.into()),
        }
    }
    // 按目标文件系统本身的大小写和 Unicode 文件名规则检查冲突。
    let probe = tempfile::tempdir_in(parent)?;
    for (relative, folder) in paths {
        let path = probe.path().join(relative);
        let result = if folder {
            std::fs::create_dir(path)
        } else {
            std::fs::File::create_new(path).map(drop)
        };
        if let Err(error) = result {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                return Err(invalid(format!(
                    "多个远程条目映射到同一本地路径: {}",
                    destination_root.join(relative).display()
                ))
                .into());
            }
            return Err(error.into());
        }
    }
    Ok(())
}

pub(crate) fn rel_to_local(root: &Path, rel_path: &str) -> PathBuf {
    rel_path
        .split('/')
        .filter(|part| !part.is_empty())
        .fold(root.to_path_buf(), |path, part| path.join(part))
}
