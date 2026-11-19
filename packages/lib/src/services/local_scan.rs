use super::sync::paths::{is_excluded, path_and_ancestors};
use crate::models::sync_item::{FileEntry, SyncEntryKind};
use crate::utils::path::{check_destination_collisions, safe_local};
use crate::utils::remote_path;
use anyhow::{Result, anyhow};
use glob::Pattern;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

pub(super) fn scan_items(
    root: &Path,
    recursive: bool,
    patterns: &[Pattern],
) -> Result<Vec<FileEntry>> {
    Ok(scan_local_snapshot(root, recursive, patterns)?.items)
}

pub(super) fn scan_upload_source(
    path: PathBuf,
    recursive: bool,
    patterns: &[Pattern],
    delete: bool,
) -> Result<(PathBuf, Vec<FileEntry>)> {
    let scan = scan_local_snapshot(&path, recursive, patterns)?;
    anyhow::ensure!(!scan.single_file || !delete, "单文件同步不支持 --delete");
    let root = if scan.single_file {
        path.parent()
            .filter(|dir| !dir.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
            .to_owned()
    } else {
        path
    };
    for entry in &scan.items {
        for name in entry.rel_path.split('/') {
            remote_path::validate_name(name)?;
        }
    }
    Ok((root, scan.items))
}

#[derive(Default)]
pub(super) struct LocalSyncScan {
    single_file: bool,
    pub(super) items: Vec<FileEntry>,
    pub(super) protected: BTreeSet<String>,
    paths: BTreeMap<String, fs::Metadata>,
}

impl LocalSyncScan {
    pub(super) fn contains_file(&self, rel_path: &str) -> bool {
        self.paths.get(rel_path).is_some_and(fs::Metadata::is_file)
    }

    pub(super) fn confirm(&self, rel_path: &str, path: &Path) -> Result<fs::Metadata> {
        safe_local(path)?;
        let original = self
            .paths
            .get(rel_path)
            .ok_or_else(|| anyhow!("本地扫描未记录待删除条目"))?;
        let current = fs::symlink_metadata(path)?;
        anyhow::ensure!(
            same_local_object(original, &current),
            "本地条目在扫描后已变更: {}",
            path.display()
        );
        Ok(current)
    }

    pub(super) fn validate_destinations(&self, source: &[FileEntry], root: &Path) -> Result<()> {
        if source.is_empty() {
            return Ok(());
        }
        let mut paths: BTreeMap<&str, bool> = self
            .paths
            .iter()
            .map(|(path, metadata)| (path.as_str(), metadata.is_dir()))
            .collect();
        for entry in source {
            let folder = entry.kind == SyncEntryKind::Directory;
            paths
                .entry(&entry.rel_path)
                .and_modify(|is_dir| *is_dir |= folder)
                .or_insert(folder);
        }
        check_destination_collisions(
            root,
            paths
                .into_iter()
                .map(|(path, folder)| (Path::new(path), folder)),
        )
    }
}

fn same_local_object(original: &fs::Metadata, current: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if (original.dev(), original.ino()) != (current.dev(), current.ino()) {
            return false;
        }
    }
    #[cfg(not(unix))]
    if original.created().ok() != current.created().ok() {
        return false;
    }
    original.is_dir() && current.is_dir()
        || original.is_file()
            && current.is_file()
            && original.len() == current.len()
            && original.modified().ok() == current.modified().ok()
}

pub(super) fn scan_local_target(
    root: &Path,
    recursive: bool,
    patterns: &[Pattern],
) -> Result<LocalSyncScan> {
    safe_local(root)?;
    match fs::symlink_metadata(root) {
        Ok(metadata) if metadata.is_dir() => scan_local_snapshot(root, recursive, patterns),
        Ok(_) => Err(anyhow!("本地同步目标必须是目录: {}", root.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(LocalSyncScan::default()),
        Err(error) => Err(error.into()),
    }
}

fn scan_local_snapshot(
    root: &Path,
    recursive: bool,
    patterns: &[Pattern],
) -> Result<LocalSyncScan> {
    safe_local(root)?;
    let mut scan = LocalSyncScan::default();
    let metadata = fs::symlink_metadata(root)?;
    if metadata.is_file() {
        scan.single_file = true;
        let name = root
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| anyhow!("无法读取文件名: {}", root.display()))?;
        if !is_excluded(name, patterns) {
            scan.items.push(local_entry(name.to_owned(), &metadata));
        }
        return Ok(scan);
    }

    scan_local_dir(root, root, recursive, patterns, &mut scan)?;
    scan.items
        .sort_by(|left, right| left.rel_path.cmp(&right.rel_path));
    Ok(scan)
}

fn scan_local_dir(
    root: &Path,
    dir: &Path,
    recursive: bool,
    patterns: &[Pattern],
    scan: &mut LocalSyncScan,
) -> Result<bool> {
    let mut empty = true;
    for entry in fs::read_dir(dir)? {
        empty = false;
        let path = entry?.path();
        let rel_path = path
            .strip_prefix(root)?
            .components()
            .map(|part| part.as_os_str().to_str())
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| anyhow!("无法读取文件名: {}", path.display()))?
            .join("/");
        let metadata = fs::symlink_metadata(&path)?;
        if is_excluded(&rel_path, patterns) || (metadata.is_dir() && !recursive) {
            scan.protected
                .extend(path_and_ancestors(&rel_path).map(str::to_owned));
        } else if metadata.is_dir() {
            let before = scan.items.len();
            let empty = scan_local_dir(root, &path, recursive, patterns, scan)?;
            if empty || scan.items.len() > before {
                scan.items.push(local_entry(rel_path.clone(), &metadata));
            }
        } else if metadata.is_file() {
            scan.items.push(local_entry(rel_path.clone(), &metadata));
        } else {
            anyhow::bail!("不支持的本地文件类型或符号链接: {}", path.display());
        }
        scan.paths.insert(rel_path, metadata);
    }
    Ok(empty)
}

fn local_entry(rel_path: String, metadata: &fs::Metadata) -> FileEntry {
    FileEntry {
        rel_path,
        size: if metadata.is_file() {
            metadata.len()
        } else {
            0
        },
        mtime: metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs() as i64),
        kind: if metadata.is_dir() {
            SyncEntryKind::Directory
        } else {
            SyncEntryKind::File
        },
    }
}
