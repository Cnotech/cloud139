use crate::domain::{
    ChangeKind, FileEntry, SyncAction, SyncDirection, SyncEndpoint, SyncEntryKind, SyncTarget,
};
use crate::{debug, warn};
use anyhow::{Result, anyhow};
use glob::Pattern;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

#[derive(Debug, Clone)]
pub struct SyncDiffOptions {
    pub direction: SyncDirection,
    pub delete: bool,
    pub checksum: bool,
    pub local_root: PathBuf,
    pub cloud_root: String,
}

pub fn parse_sync_endpoint(value: &str) -> SyncEndpoint {
    if let Some(rest) = value.strip_prefix("cloud:") {
        return SyncEndpoint::Cloud(normalize_cloud_path(rest));
    }
    SyncEndpoint::Local(PathBuf::from(value))
}

pub fn resolve_sync_direction(src: &str, dest: &str) -> Result<SyncDirection> {
    match (parse_sync_endpoint(src), parse_sync_endpoint(dest)) {
        (SyncEndpoint::Local(_), SyncEndpoint::Cloud(_)) => Ok(SyncDirection::LocalToCloud),
        (SyncEndpoint::Cloud(_), SyncEndpoint::Local(_)) => Ok(SyncDirection::CloudToLocal),
        (SyncEndpoint::Local(_), SyncEndpoint::Local(_)) => {
            Err(anyhow!("SRC 和 DEST 都是本地路径，请使用 cp/mv 或系统工具"))
        }
        (SyncEndpoint::Cloud(_), SyncEndpoint::Cloud(_)) => {
            Err(anyhow!("SRC 和 DEST 都是云端路径，请使用 cp/mv 命令"))
        }
    }
}

pub fn normalize_cloud_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() || trimmed == "/" {
        return "/".to_string();
    }
    format!("/{}", trimmed.trim_start_matches('/').trim_end_matches('/'))
}

fn join_cloud_path(root: &str, rel_path: &str) -> String {
    if root == "/" {
        format!("/{}", rel_path.trim_start_matches('/'))
    } else {
        format!(
            "{}/{}",
            root.trim_end_matches('/'),
            rel_path.trim_start_matches('/')
        )
    }
}

fn rel_to_local(root: &Path, rel_path: &str) -> PathBuf {
    rel_path
        .split('/')
        .filter(|part| !part.is_empty())
        .fold(root.to_path_buf(), |path, part| path.join(part))
}

pub fn compute_diff(
    source: &[FileEntry],
    target: &[FileEntry],
    options: SyncDiffOptions,
) -> Vec<SyncAction> {
    debug!("compute_diff: source={} entries, target={} entries, direction={:?}",
        source.len(), target.len(), options.direction);
    let source_by_path: BTreeMap<&str, &FileEntry> = source
        .iter()
        .map(|item| (item.rel_path.as_str(), item))
        .collect();
    let target_by_path: BTreeMap<&str, &FileEntry> = target
        .iter()
        .map(|item| (item.rel_path.as_str(), item))
        .collect();

    let mut paths = BTreeSet::new();
    paths.extend(source_by_path.keys().copied());
    paths.extend(target_by_path.keys().copied());

    let mut actions = Vec::new();

    for path in paths {
        match (source_by_path.get(path), target_by_path.get(path)) {
            (Some(src), Some(dst)) if src.kind != dst.kind => {
                // Type conflict: file vs directory - always treat as change
                match src.kind {
                    SyncEntryKind::Directory => {
                        // Source is directory, target is file -> create dir (file will be overwritten)
                        actions.push(create_dir_action(src, &options));
                    }
                    SyncEntryKind::File => {
                        // Source is file, target is directory -> upload file
                        actions.push(transfer_action(src, &options, ChangeKind::New));
                    }
                }
            }
            (Some(src), Some(dst)) => {
                if let Some(change) = change_kind(src, dst, options.checksum, options.direction) {
                    actions.push(transfer_action(src, &options, change));
                } else {
                    actions.push(SyncAction::Skip {
                        rel_path: (*path).to_string(),
                    });
                }
            }
            (Some(src), None) if src.kind == SyncEntryKind::Directory => {
                actions.push(create_dir_action(src, &options));
            }
            (Some(src), None) => actions.push(transfer_action(src, &options, ChangeKind::New)),
            (None, Some(dst)) if options.delete && dst.kind == SyncEntryKind::Directory => {
                actions.push(delete_action(dst, &options));
            }
            (None, Some(dst)) if options.delete => actions.push(delete_action(dst, &options)),
            (None, Some(_dst)) => actions.push(SyncAction::Skip {
                rel_path: (*path).to_string(),
            }),
            (None, None) => {}
        }
    }

    actions.sort_by_key(|action| match action {
        SyncAction::CreateDir { rel_path, .. } => (0, rel_path.matches('/').count(), rel_path.clone()),
        SyncAction::Upload { rel_path, .. } | SyncAction::Download { rel_path, .. } => {
            (1, rel_path.matches('/').count(), rel_path.clone())
        }
        SyncAction::Skip { rel_path } => (2, rel_path.matches('/').count(), rel_path.clone()),
        SyncAction::Delete { rel_path, .. } => (3, usize::MAX - rel_path.matches('/').count(), rel_path.clone()),
    });

    debug!("compute_diff: 生成 {} 个动作", actions.len());
    actions
}

fn change_kind(
    source: &FileEntry,
    target: &FileEntry,
    checksum: bool,
    direction: SyncDirection,
) -> Option<ChangeKind> {
    if source.size != target.size {
        return Some(ChangeKind::SizeOrTime);
    }

    // LocalToCloud: cloud mtime reflects upload time, not the local file's
    // original mtime, so mtime comparison is unreliable. Compare sizes only,
    // unless both sides have checksums and checksum mode is enabled.
    if direction == SyncDirection::LocalToCloud {
        if checksum && let (Some(s), Some(t)) = (&source.checksum, &target.checksum) {
            return if s != t {
                Some(ChangeKind::Checksum)
            } else {
                None
            };
        }
        // One or both checksums missing; mtime is unreliable for
        // LocalToCloud. Fall back to size-only comparison.
        if checksum {
            warn!(
                "checksum 模式下云端缺少 checksum，回退到仅按文件大小比较: {}",
                source.rel_path
            );
        }
        return None;
    }

    // CloudToLocal (or generic): compare checksums when both sides have them.
    if checksum && let (Some(s), Some(t)) = (&source.checksum, &target.checksum) {
        return if s != t {
            Some(ChangeKind::Checksum)
        } else {
            None
        };
    }

    match (source.mtime, target.mtime) {
        (Some(left), Some(right)) if (left - right).abs() > 2 => Some(ChangeKind::SizeOrTime),
        (Some(_), Some(_)) => None,
        (None, None) => None,
        _ => Some(ChangeKind::SizeOrTime),
    }
}

fn transfer_action(entry: &FileEntry, options: &SyncDiffOptions, change: ChangeKind) -> SyncAction {
    match options.direction {
        SyncDirection::LocalToCloud => SyncAction::Upload {
            rel_path: entry.rel_path.clone(),
            local_abs: rel_to_local(&options.local_root, &entry.rel_path),
            remote_abs: join_cloud_path(&options.cloud_root, &entry.rel_path),
            size: entry.size,
            change,
        },
        SyncDirection::CloudToLocal => SyncAction::Download {
            rel_path: entry.rel_path.clone(),
            remote_abs: join_cloud_path(&options.cloud_root, &entry.rel_path),
            local_abs: rel_to_local(&options.local_root, &entry.rel_path),
            size: entry.size,
            change,
            cloud_mtime: entry.mtime,
        },
    }
}

fn delete_action(entry: &FileEntry, options: &SyncDiffOptions) -> SyncAction {
    match options.direction {
        SyncDirection::LocalToCloud => SyncAction::Delete {
            rel_path: entry.rel_path.clone(),
            target: SyncTarget::Cloud,
            target_abs: join_cloud_path(&options.cloud_root, &entry.rel_path),
        },
        SyncDirection::CloudToLocal => SyncAction::Delete {
            rel_path: entry.rel_path.clone(),
            target: SyncTarget::Local,
            target_abs: rel_to_local(&options.local_root, &entry.rel_path)
                .to_string_lossy()
                .to_string(),
        },
    }
}

fn create_dir_action(entry: &FileEntry, options: &SyncDiffOptions) -> SyncAction {
    match options.direction {
        SyncDirection::LocalToCloud => SyncAction::CreateDir {
            rel_path: entry.rel_path.clone(),
            target: SyncTarget::Cloud,
            target_abs: join_cloud_path(&options.cloud_root, &entry.rel_path),
        },
        SyncDirection::CloudToLocal => SyncAction::CreateDir {
            rel_path: entry.rel_path.clone(),
            target: SyncTarget::Local,
            target_abs: rel_to_local(&options.local_root, &entry.rel_path)
                .to_string_lossy()
                .to_string(),
        },
    }
}

pub fn format_action_line(action: &SyncAction, dry_run: bool) -> String {
    let prefix = if dry_run { "(DRY RUN) " } else { "" };
    match action {
        SyncAction::Upload {
            rel_path, change, ..
        }
        | SyncAction::Download {
            rel_path, change, ..
        } => {
            format!("{}{} {}", prefix, change_marker(*change), rel_path)
        }
        SyncAction::CreateDir { rel_path, .. } => format!("{}cd+++++++++ {}", prefix, rel_path),
        SyncAction::Delete { rel_path, .. } => format!("{}*deleting   {}", prefix, rel_path),
        SyncAction::Skip { rel_path } => format!("{}skipping    {}", prefix, rel_path),
    }
}

fn change_marker(change: ChangeKind) -> &'static str {
    match change {
        ChangeKind::New => ">f+++++++++",
        ChangeKind::SizeOrTime => ">f.st......",
        ChangeKind::Checksum => ">f.c.......",
    }
}

#[derive(Debug, Clone)]
pub struct SyncScanOptions {
    pub recursive: bool,
    pub checksum: bool,
    pub exclude: Vec<String>,
}

pub fn scan_local(root: &Path, options: SyncScanOptions) -> Result<Vec<FileEntry>> {
    if !root.exists() {
        return Err(anyhow!("本地路径不存在: {}", root.display()));
    }

    let patterns = compile_exclude_patterns(&options.exclude)?;
    let mut items = Vec::new();

    if root.is_file() {
        let file_name = root
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| anyhow!("无法读取文件名: {}", root.display()))?;
        if !is_excluded(file_name, &patterns) {
            items.push(file_entry_from_local(
                root,
                file_name.to_string(),
                options.checksum,
            )?);
        }
        return Ok(items);
    }

    scan_local_dir(root, root, &options, &patterns, &mut items)?;
    items.sort_by(|left, right| left.rel_path.cmp(&right.rel_path));
    Ok(items)
}

fn scan_local_dir(
    root: &Path,
    dir: &Path,
    options: &SyncScanOptions,
    patterns: &[Pattern],
    items: &mut Vec<FileEntry>,
) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let rel_path = to_forward_slash(path.strip_prefix(root)?);

        if is_excluded(&rel_path, patterns) {
            continue;
        }

        let metadata = entry.metadata()?;
        if metadata.is_dir() {
            if options.recursive {
                // Remember position before scanning children
                let items_before = items.len();
                scan_local_dir(root, &path, options, patterns, items)?;

                // Add directory if:
                // 1. It has non-excluded children (items.len() > items_before), OR
                // 2. It's truly empty (no entries at all)
                let is_truly_empty = dir_is_truly_empty(&path)?;
                if items.len() > items_before || is_truly_empty {
                    items.insert(
                        items_before,
                        FileEntry {
                            rel_path: rel_path.clone(),
                            size: 0,
                            mtime: metadata
                                .modified()
                                .ok()
                                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                                .map(|duration| duration.as_secs() as i64),
                            checksum: None,
                            kind: SyncEntryKind::Directory,
                        },
                    );
                }
            }
            continue;
        }

        if metadata.is_file() {
            items.push(file_entry_from_local(&path, rel_path, options.checksum)?);
        }
    }

    Ok(())
}

fn dir_is_truly_empty(dir: &Path) -> Result<bool> {
    // Check if directory has any entries at all (regardless of exclusion)
    let mut entries = fs::read_dir(dir)?;
    Ok(entries.next().is_none())
}

fn file_entry_from_local(path: &Path, rel_path: String, checksum: bool) -> Result<FileEntry> {
    let metadata = fs::metadata(path)?;
    let mtime = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs() as i64);

    Ok(FileEntry {
        rel_path,
        size: metadata.len(),
        mtime,
        checksum: if checksum {
            Some(sha256_file(path)?)
        } else {
            None
        },
        kind: SyncEntryKind::File,
    })
}

fn compile_exclude_patterns(exclude: &[String]) -> Result<Vec<Pattern>> {
    exclude
        .iter()
        .map(|pattern| {
            Pattern::new(pattern).map_err(|err| anyhow!("无效 exclude glob `{}`: {}", pattern, err))
        })
        .collect()
}

fn is_excluded(rel_path: &str, patterns: &[Pattern]) -> bool {
    let options = glob::MatchOptions {
        case_sensitive: true,
        require_literal_separator: true,
        require_literal_leading_dot: false,
    };
    patterns.iter().any(|pattern| {
        // Direct match
        if pattern.matches_with(rel_path, options) {
            return true;
        }
        // Also check if pattern like "dir/**" matches "dir" (the directory itself)
        // This ensures --exclude target/** also excludes the target directory
        let pattern_str = pattern.as_str();
        if let Some(prefix) = pattern_str.strip_suffix("/**")
            && rel_path == prefix
        {
            return true;
        }
        false
    })
}

fn to_forward_slash(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn sha256_file(path: &Path) -> Result<String> {
    let path_str = path
        .to_str()
        .ok_or_else(|| anyhow!("路径不是合法 UTF-8: {}", path.display()))?;
    Ok(crate::utils::crypto::calc_file_sha256(path_str)?)
}

pub fn should_treat_personal_item_as_folder(item: &crate::models::PersonalFileItem) -> bool {
    matches!(
        item.file_type.as_deref(),
        Some("folder") | Some("dir") | Some("1")
    )
}

pub fn cloud_child_path(parent: &str, name: &str) -> String {
    if parent == "/" || parent.is_empty() {
        format!("/{}", name.trim_matches('/'))
    } else {
        format!(
            "{}/{}",
            parent.trim_end_matches('/'),
            name.trim_matches('/')
        )
    }
}

pub fn personal_item_to_file_entry(
    rel_parent: &str,
    item: &crate::models::PersonalFileItem,
    checksum: bool,
) -> Result<FileEntry> {
    let name = item
        .name
        .as_deref()
        .ok_or_else(|| anyhow!("云端文件缺少名称"))?;
    let rel_path = if rel_parent.is_empty() {
        name.to_string()
    } else {
        format!("{}/{}", rel_parent.trim_matches('/'), name)
    };

    let checksum = if checksum
        && item.content_hash_algorithm.as_deref() == Some("SHA256")
    {
        item.content_hash.clone().map(|value| value.to_ascii_lowercase())
    } else {
        None
    };

    Ok(FileEntry {
        rel_path,
        size: item.size.unwrap_or(0).max(0) as u64,
        mtime: parse_cloud_mtime(item),
        checksum,
        kind: SyncEntryKind::File,
    })
}

fn parse_cloud_mtime(item: &crate::models::PersonalFileItem) -> Option<i64> {
    let raw = item
        .updated_at
        .as_deref()
        .or(item.update_date.as_deref())
        .or(item.last_modified.as_deref())?;

    chrono::DateTime::parse_from_rfc3339(raw)
        .map(|time| time.timestamp())
        .ok()
}

pub async fn scan_cloud_personal(
    config: &crate::config::Config,
    cloud_root: &str,
    options: SyncScanOptions,
) -> Result<Vec<FileEntry>> {
    let mut config = config.clone();
    let host = match &config.personal_cloud_host {
        Some(host) => host.clone(),
        None => crate::client::api::get_personal_cloud_host(&mut config).await?,
    };

    let root_id = if cloud_root == "/" {
        "/".to_string()
    } else {
        crate::client::api::get_file_id_by_path(&config, cloud_root).await?
    };

    let patterns = compile_exclude_patterns(&options.exclude)?;
    let mut items = Vec::new();
    scan_cloud_personal_dir(
        &config, &host, cloud_root, "", &root_id, &options, &patterns, &mut items,
    )
    .await?;
    items.sort_by(|left, right| left.rel_path.cmp(&right.rel_path));
    Ok(items)
}

#[allow(clippy::too_many_arguments)]
async fn scan_cloud_personal_dir(
    config: &crate::config::Config,
    host: &str,
    abs_parent: &str,
    rel_parent: &str,
    parent_file_id: &str,
    options: &SyncScanOptions,
    patterns: &[Pattern],
    items: &mut Vec<FileEntry>,
) -> Result<()> {
    scan_cloud_personal_dir_inner(
        config,
        host,
        abs_parent,
        rel_parent,
        parent_file_id,
        options,
        patterns,
        items,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
fn scan_cloud_personal_dir_inner<'a>(
    config: &'a crate::config::Config,
    host: &'a str,
    abs_parent: &'a str,
    rel_parent: &'a str,
    parent_file_id: &'a str,
    options: &'a SyncScanOptions,
    patterns: &'a [Pattern],
    items: &'a mut Vec<FileEntry>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>> {
    Box::pin(async move {
        let url = format!("{}/file/list", host);
        let mut next_cursor = String::new();

        loop {
            let body = serde_json::json!({
                "parentFileId": parent_file_id,
                "pageInfo": {
                    "pageCursor": next_cursor,
                    "pageSize": 100
                },
                "orderBy": "updated_at",
                "orderDirection": "DESC"
            });

            let resp: crate::models::PersonalListResp = crate::client::api::personal_api_request(
                config,
                &url,
                body,
                crate::client::StorageType::PersonalNew,
            )
            .await?;

            if !resp.base.success {
                return Err(crate::client::ClientError::Api(
                    resp.base
                        .message
                        .unwrap_or_else(|| "获取云端文件列表失败".to_string()),
                )
                .into());
            }

            let data = resp
                .data
                .ok_or_else(|| anyhow!("获取云端文件列表失败: 无数据"))?;

            for item in data.items {
                let name = item.name.clone().unwrap_or_default();
                if name.is_empty() {
                    continue;
                }
                let rel_path = if rel_parent.is_empty() {
                    name.clone()
                } else {
                    format!("{}/{}", rel_parent, name)
                };
                if is_excluded(&rel_path, patterns) {
                    continue;
                }

                if should_treat_personal_item_as_folder(&item) {
                    if options.recursive {
                        let child_id = item.file_id.clone().unwrap_or_default();
                        if !child_id.is_empty() {
                            items.push(FileEntry {
                                rel_path: rel_path.clone(),
                                size: 0,
                                mtime: parse_cloud_mtime(&item),
                                checksum: None,
                                kind: SyncEntryKind::Directory,
                            });
                            scan_cloud_personal_dir(
                                config,
                                host,
                                &cloud_child_path(abs_parent, &name),
                                &rel_path,
                                &child_id,
                                options,
                                patterns,
                                items,
                            )
                            .await?;
                        }
                    }
                } else {
                    let mut entry = personal_item_to_file_entry(rel_parent, &item, options.checksum)?;

                    if options.checksum && entry.checksum.is_none()
                        && let Some(file_id) = item.file_id.as_deref()
                        && let Ok(detail) =
                            crate::client::api::get_personal_file_detail(config, file_id).await
                        && detail.content_hash_algorithm.as_deref() == Some("SHA256")
                    {
                        entry.checksum =
                            detail.content_hash.map(|value| value.to_ascii_lowercase());
                    }

                    items.push(entry);
                }
            }

            next_cursor = data.next_page_cursor.unwrap_or_default();
            if next_cursor.is_empty() {
                break;
            }
        }

        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_excluded_double_star_matches_nested_path() {
        let patterns = compile_exclude_patterns(&["target/**".to_string()]).unwrap();
        assert!(is_excluded("target/debug/app", &patterns));
        assert!(is_excluded("target/build", &patterns));
        assert!(!is_excluded("src/main.rs", &patterns));
    }

    #[test]
    fn test_is_excluded_single_star_does_not_cross_dirs() {
        let patterns = compile_exclude_patterns(&["*.log".to_string()]).unwrap();
        assert!(is_excluded("app.log", &patterns));
        assert!(is_excluded("debug.log", &patterns));
        assert!(!is_excluded("dir/app.log", &patterns));
    }
}
