use crate::debug;
use crate::domain::{
    ChangeKind, FileEntry, SyncAction, SyncDirection, SyncEndpoint, SyncEntryKind, SyncTarget,
};
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
    pub local_root: PathBuf,
    pub cloud_root: String,
    /// --verify：LocalToCloud 时按 size + contentHash 判定一致性（默认只比 size）。
    pub verify: bool,
}

/// compute_diff 的附带统计，仅 --verify + --json 使用；目录不计入。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyncDiffStats {
    /// 源端（本地）参与同步的文件数。
    pub objects: usize,
    /// 已存在且校验通过（或按文件名保证）的文件数。
    pub skipped_files: usize,
    /// 远端未返回可比 contentHash、按文件名保证的内容寻址对象数。
    pub hash_missing: usize,
    /// --delete 时计划删除的远端多余文件数。
    pub planned_deleted_files: usize,
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
    compute_diff_with_stats(source, target, options).0
}

/// 与 compute_diff 相同，但额外返回 --verify/--json 需要的文件口径统计。
pub fn compute_diff_with_stats(
    source: &[FileEntry],
    target: &[FileEntry],
    options: SyncDiffOptions,
) -> (Vec<SyncAction>, SyncDiffStats) {
    debug!(
        "compute_diff: source={} entries, target={} entries, direction={:?}, verify={}",
        source.len(),
        target.len(),
        options.direction,
        options.verify
    );
    let source_by_path: BTreeMap<&str, &FileEntry> = source
        .iter()
        .map(|item| (item.rel_path.as_str(), item))
        .collect();
    let target_by_path: BTreeMap<&str, &FileEntry> = target
        .iter()
        .map(|item| (item.rel_path.as_str(), item))
        .collect();
    // 云端条目（scan 已带 file_id）的 rel_path -> fileId。用于给动作补齐父目录/旧文件 id，
    // 执行阶段就不再逐文件按路径解析。
    let cloud_ids: BTreeMap<&str, &str> = source
        .iter()
        .chain(target.iter())
        .filter_map(|item| {
            item.file_id
                .as_deref()
                .filter(|id| !id.is_empty())
                .map(|id| (item.rel_path.as_str(), id))
        })
        .collect();

    let mut stats = SyncDiffStats::default();
    for _ in source.iter().filter(|e| e.kind == SyncEntryKind::File) {
        stats.objects += 1;
    }
    if options.delete {
        for dst in target.iter().filter(|e| e.kind == SyncEntryKind::File) {
            if !source_by_path.contains_key(dst.rel_path.as_str()) {
                stats.planned_deleted_files += 1;
            }
        }
    }

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
                        actions.push(create_dir_action(src, &options, &cloud_ids));
                    }
                    SyncEntryKind::File => {
                        // Source is file, target is directory -> upload file
                        let parent = parent_file_id_of(&src.rel_path, &cloud_ids);
                        actions.push(transfer_action(
                            src,
                            &options,
                            ChangeKind::New,
                            parent,
                            None,
                        ));
                    }
                }
            }
            (Some(src), Some(dst)) => {
                let change = if options.verify && options.direction == SyncDirection::LocalToCloud {
                    match classify_verify_local(src, dst, &options.local_root) {
                        VerifyClass::Unchanged => {
                            if src.kind == SyncEntryKind::File {
                                stats.skipped_files += 1;
                            }
                            None
                        }
                        VerifyClass::UnchangedHashMissing => {
                            if src.kind == SyncEntryKind::File {
                                stats.skipped_files += 1;
                                stats.hash_missing += 1;
                            }
                            None
                        }
                        VerifyClass::Replace => Some(ChangeKind::SizeOrTime),
                    }
                } else {
                    change_kind(src, dst, options.direction)
                };

                match change {
                    Some(change) => {
                        let parent = parent_file_id_of(&src.rel_path, &cloud_ids);
                        let existing = match options.direction {
                            SyncDirection::LocalToCloud => dst.file_id.clone(),
                            SyncDirection::CloudToLocal => src.file_id.clone(),
                        };
                        actions.push(transfer_action(src, &options, change, parent, existing));
                    }
                    None => actions.push(SyncAction::Skip {
                        rel_path: (*path).to_string(),
                    }),
                }
            }
            (Some(src), None) if src.kind == SyncEntryKind::Directory => {
                actions.push(create_dir_action(src, &options, &cloud_ids));
            }
            (Some(src), None) => {
                let parent = parent_file_id_of(&src.rel_path, &cloud_ids);
                actions.push(transfer_action(
                    src,
                    &options,
                    ChangeKind::New,
                    parent,
                    None,
                ));
            }
            (None, Some(dst)) if options.delete => {
                actions.push(delete_action(dst, &options));
            }
            (None, Some(_dst)) => actions.push(SyncAction::Skip {
                rel_path: (*path).to_string(),
            }),
            (None, None) => {}
        }
    }

    actions.sort_by_key(|action| match action {
        SyncAction::CreateDir { rel_path, .. } => {
            (0, rel_path.matches('/').count(), rel_path.clone())
        }
        SyncAction::Upload { rel_path, .. } | SyncAction::Download { rel_path, .. } => {
            (1, rel_path.matches('/').count(), rel_path.clone())
        }
        SyncAction::Skip { rel_path } => (2, rel_path.matches('/').count(), rel_path.clone()),
        SyncAction::Delete { rel_path, .. } => (
            3,
            usize::MAX - rel_path.matches('/').count(),
            rel_path.clone(),
        ),
    });

    debug!("compute_diff: 生成 {} 个动作", actions.len());
    (actions, stats)
}

/// 路径的父目录在云端扫描结果里的 fileId；顶层路径（父为 cloud root）返回 None。
fn parent_file_id_of(path: &str, cloud_ids: &BTreeMap<&str, &str>) -> Option<String> {
    let (parent, _) = path.rsplit_once('/')?;
    if parent.is_empty() {
        return None;
    }
    cloud_ids.get(parent).map(|id| (*id).to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VerifyClass {
    Unchanged,
    UnchangedHashMissing,
    Replace,
}

/// --verify（LocalToCloud）的同名文件判定：64-hex 对象按文件名即 sha256，
/// 普通文件现算本地 sha256；size 或可比 contentHash 不一致 -> 替换。
/// 远端缺哈希时：内容寻址对象按文件名保证（skip + hashMissing），普通文件 -> 替换。
fn classify_verify_local(src: &FileEntry, dst: &FileEntry, local_root: &Path) -> VerifyClass {
    if src.kind == SyncEntryKind::Directory || dst.kind == SyncEntryKind::Directory {
        return VerifyClass::Unchanged;
    }
    if src.size != dst.size {
        return VerifyClass::Replace;
    }

    let content_addressed = is_content_addressed(&src.rel_path);
    let has_comparable_hash =
        dst.content_hash.is_some() && is_sha256_algorithm(dst.content_hash_algorithm.as_deref());
    if has_comparable_hash {
        let abs_path = rel_to_local(local_root, &src.rel_path);
        let expected = match expected_sha256(&src.rel_path, &abs_path) {
            Some(hash) => hash,
            None => return VerifyClass::Replace,
        };
        if dst
            .content_hash
            .as_deref()
            .is_some_and(|hash| hash_matches(hash, &expected))
        {
            VerifyClass::Unchanged
        } else {
            VerifyClass::Replace
        }
    } else if content_addressed {
        VerifyClass::UnchangedHashMissing
    } else {
        VerifyClass::Replace
    }
}

/// 文件名是 64 位十六进制 = 内容寻址对象，名字本身即内容 sha256。
pub(crate) fn is_content_addressed(rel_path: &str) -> bool {
    let name = rel_path.rsplit('/').next().unwrap_or(rel_path);
    name.len() == 64 && name.chars().all(|c| c.is_ascii_hexdigit())
}

/// 期望的内容 sha256（统一小写）：内容寻址对象用文件名，其它文件现算本地 sha256。
/// 读取失败返回 None（调用方按「无法确认」处理）。
pub(crate) fn expected_sha256(rel_path: &str, abs_path: &Path) -> Option<String> {
    if is_content_addressed(rel_path) {
        let name = rel_path.rsplit('/').next().unwrap_or(rel_path);
        return Some(name.to_lowercase());
    }
    crate::utils::crypto::calc_file_sha256(&abs_path.to_string_lossy())
        .ok()
        .map(|hash| hash.to_lowercase())
}

pub(crate) fn is_sha256_algorithm(algorithm: Option<&str>) -> bool {
    algorithm
        .map(|value| value.trim().eq_ignore_ascii_case("SHA256"))
        .unwrap_or(false)
}

pub(crate) fn hash_matches(remote_hash: &str, expected: &str) -> bool {
    remote_hash.trim().to_lowercase() == expected.trim().to_lowercase()
}

fn change_kind(
    source: &FileEntry,
    target: &FileEntry,
    direction: SyncDirection,
) -> Option<ChangeKind> {
    // Directories don't need re-creation; their content changes are handled by children
    if source.kind == SyncEntryKind::Directory && target.kind == SyncEntryKind::Directory {
        return None;
    }

    if source.size != target.size {
        return Some(ChangeKind::SizeOrTime);
    }

    // LocalToCloud: cloud mtime reflects upload time, not the local file's
    // original mtime, so mtime comparison is unreliable. Compare sizes only.
    if direction == SyncDirection::LocalToCloud {
        return None;
    }

    match (source.mtime, target.mtime) {
        (Some(left), Some(right)) if (left - right).abs() > 2 => Some(ChangeKind::SizeOrTime),
        (Some(_), Some(_)) => None,
        (None, None) => None,
        _ => Some(ChangeKind::SizeOrTime),
    }
}

fn transfer_action(
    entry: &FileEntry,
    options: &SyncDiffOptions,
    change: ChangeKind,
    parent_file_id: Option<String>,
    file_id: Option<String>,
) -> SyncAction {
    match options.direction {
        SyncDirection::LocalToCloud => SyncAction::Upload {
            rel_path: entry.rel_path.clone(),
            local_abs: rel_to_local(&options.local_root, &entry.rel_path),
            remote_abs: join_cloud_path(&options.cloud_root, &entry.rel_path),
            size: entry.size,
            change,
            parent_file_id,
            file_id,
        },
        SyncDirection::CloudToLocal => SyncAction::Download {
            rel_path: entry.rel_path.clone(),
            remote_abs: join_cloud_path(&options.cloud_root, &entry.rel_path),
            local_abs: rel_to_local(&options.local_root, &entry.rel_path),
            size: entry.size,
            change,
            cloud_mtime: entry.mtime,
            file_id,
            parent_file_id,
        },
    }
}

fn delete_action(entry: &FileEntry, options: &SyncDiffOptions) -> SyncAction {
    match options.direction {
        SyncDirection::LocalToCloud => SyncAction::Delete {
            rel_path: entry.rel_path.clone(),
            target: SyncTarget::Cloud,
            target_abs: join_cloud_path(&options.cloud_root, &entry.rel_path),
            file_id: entry.file_id.clone().filter(|id| !id.is_empty()),
            kind: entry.kind,
        },
        SyncDirection::CloudToLocal => SyncAction::Delete {
            rel_path: entry.rel_path.clone(),
            target: SyncTarget::Local,
            target_abs: rel_to_local(&options.local_root, &entry.rel_path)
                .to_string_lossy()
                .to_string(),
            file_id: None,
            kind: entry.kind,
        },
    }
}

fn create_dir_action(
    entry: &FileEntry,
    options: &SyncDiffOptions,
    cloud_ids: &BTreeMap<&str, &str>,
) -> SyncAction {
    match options.direction {
        SyncDirection::LocalToCloud => SyncAction::CreateDir {
            rel_path: entry.rel_path.clone(),
            target: SyncTarget::Cloud,
            target_abs: join_cloud_path(&options.cloud_root, &entry.rel_path),
            parent_file_id: parent_file_id_of(&entry.rel_path, cloud_ids),
        },
        SyncDirection::CloudToLocal => SyncAction::CreateDir {
            rel_path: entry.rel_path.clone(),
            target: SyncTarget::Local,
            target_abs: rel_to_local(&options.local_root, &entry.rel_path)
                .to_string_lossy()
                .to_string(),
            parent_file_id: None,
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
    }
}

#[derive(Debug, Clone)]
pub struct SyncScanOptions {
    pub recursive: bool,
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
            items.push(file_entry_from_local(root, file_name.to_string())?);
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
                            kind: SyncEntryKind::Directory,
                            file_id: None,
                            content_hash: None,
                            content_hash_algorithm: None,
                        },
                    );
                }
            }
            continue;
        }

        if metadata.is_file() {
            items.push(file_entry_from_local(&path, rel_path)?);
        }
    }

    Ok(())
}

fn dir_is_truly_empty(dir: &Path) -> Result<bool> {
    // Check if directory has any entries at all (regardless of exclusion)
    let mut entries = fs::read_dir(dir)?;
    Ok(entries.next().is_none())
}

fn file_entry_from_local(path: &Path, rel_path: String) -> Result<FileEntry> {
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
        kind: SyncEntryKind::File,
        file_id: None,
        content_hash: None,
        content_hash_algorithm: None,
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

    Ok(FileEntry {
        rel_path,
        size: item.size.unwrap_or(0).max(0) as u64,
        mtime: parse_cloud_mtime(item),
        kind: SyncEntryKind::File,
        file_id: item.file_id.clone().filter(|id| !id.is_empty()),
        content_hash: item.content_hash.clone(),
        content_hash_algorithm: item.content_hash_algorithm.clone(),
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

/// sync 的请求级重试参数：4 次重试（首次 + 4），退避 2/4/8/16 秒。
///
/// 只作用于**读操作**（列表、下载、路径解析）；上传/建目录等写操作不重试。
pub(crate) const SYNC_RETRIES: u32 = 4;
pub(crate) const SYNC_BACKOFF_SECS: u64 = 2;

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
        crate::utils::retry::with_retries(SYNC_RETRIES, SYNC_BACKOFF_SECS, || async {
            crate::client::api::get_file_id_by_path(&config, cloud_root)
                .await
                .map_err(anyhow::Error::from)
        })
        .await?
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
        let mut seen_cursors = std::collections::HashSet::new();

        loop {
            // Keep the guard per directory, outside request retries. Retrying the
            // current page is valid; revisiting it as a new page is not.
            if !seen_cursors.insert(next_cursor.clone()) {
                return Err(crate::client::ClientError::Api(
                    "分页游标重复，已停止列目录".to_string(),
                )
                .into());
            }
            let body = serde_json::json!({
                "parentFileId": parent_file_id,
                "pageInfo": {
                    "pageCursor": next_cursor,
                    "pageSize": 100
                },
                "orderBy": "updated_at",
                "orderDirection": "DESC"
            });

            // 每个分页请求单独重试：断连就地重试，不惊动上层、不让整条命令失败。
            let resp: crate::models::PersonalListResp =
                crate::utils::retry::with_retries(SYNC_RETRIES, SYNC_BACKOFF_SECS, || async {
                    crate::client::api::personal_api_request(
                        config,
                        &url,
                        body.clone(),
                        crate::domain::StorageType::PersonalNew,
                    )
                    .await
                    .map_err(anyhow::Error::from)
                })
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
                                kind: SyncEntryKind::Directory,
                                file_id: item.file_id.clone().filter(|id| !id.is_empty()),
                                content_hash: None,
                                content_hash_algorithm: None,
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
                    let entry = personal_item_to_file_entry(rel_parent, &item)?;
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

    fn file_entry(path: &str, size: u64, file_id: Option<&str>, hash: Option<&str>) -> FileEntry {
        FileEntry {
            rel_path: path.to_string(),
            size,
            mtime: None,
            kind: SyncEntryKind::File,
            file_id: file_id.map(str::to_string),
            content_hash: hash.map(str::to_string),
            content_hash_algorithm: hash.map(|_| "SHA256".to_string()),
        }
    }

    fn dir_entry(path: &str, file_id: Option<&str>) -> FileEntry {
        FileEntry {
            rel_path: path.to_string(),
            size: 0,
            mtime: None,
            kind: SyncEntryKind::Directory,
            file_id: file_id.map(str::to_string),
            content_hash: None,
            content_hash_algorithm: None,
        }
    }

    fn verify_options(local_root: std::path::PathBuf) -> SyncDiffOptions {
        SyncDiffOptions {
            direction: SyncDirection::LocalToCloud,
            delete: false,
            local_root,
            cloud_root: "/remote".to_string(),
            verify: true,
        }
    }

    fn action_kinds(actions: &[SyncAction]) -> Vec<&'static str> {
        actions
            .iter()
            .map(|action| match action {
                SyncAction::Upload { .. } => "upload",
                SyncAction::Download { .. } => "download",
                SyncAction::CreateDir { .. } => "mkdir",
                SyncAction::Delete { .. } => "delete",
                SyncAction::Skip { .. } => "skip",
            })
            .collect()
    }

    const HELLO_SHA256: &str = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";

    #[test]
    fn content_addressed_detection() {
        assert!(is_content_addressed(&format!("data/ab/{}", "a".repeat(64))));
        assert!(is_content_addressed(&"A".repeat(64)));
        assert!(!is_content_addressed("config"));
        assert!(!is_content_addressed(&"a".repeat(63)));
    }

    #[test]
    fn hash_compare_is_case_insensitive() {
        assert!(hash_matches("ABCDEF", "abcdef"));
        assert!(!hash_matches("abcdef", "abcde0"));
    }

    #[test]
    fn verify_skips_content_addressed_object_with_matching_hash() {
        let tmp = tempfile::tempdir().unwrap();
        let name = "a".repeat(64);
        let path = format!("data/aa/{name}");
        let source = vec![file_entry(&path, 10, None, None)];
        let target = vec![file_entry(&path, 10, Some("id-1"), Some(&name))];
        let (actions, stats) =
            compute_diff_with_stats(&source, &target, verify_options(tmp.path().to_path_buf()));
        assert_eq!(action_kinds(&actions), vec!["skip"]);
        assert_eq!(stats.skipped_files, 1);
        assert_eq!(stats.hash_missing, 0);
    }

    #[test]
    fn verify_replaces_content_addressed_object_with_wrong_hash() {
        let tmp = tempfile::tempdir().unwrap();
        let name = "a".repeat(64);
        let path = format!("data/aa/{name}");
        let source = vec![file_entry(&path, 10, None, None)];
        let target = vec![file_entry(&path, 10, Some("id-1"), Some(&"b".repeat(64)))];
        let (actions, _) =
            compute_diff_with_stats(&source, &target, verify_options(tmp.path().to_path_buf()));
        assert_eq!(action_kinds(&actions), vec!["upload"]);
    }

    #[test]
    fn verify_skips_content_addressed_object_when_remote_hash_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let name = "a".repeat(64);
        let path = format!("data/aa/{name}");
        let source = vec![file_entry(&path, 10, None, None)];
        let target = vec![file_entry(&path, 10, Some("id-1"), None)];
        let (actions, stats) =
            compute_diff_with_stats(&source, &target, verify_options(tmp.path().to_path_buf()));
        assert_eq!(action_kinds(&actions), vec!["skip"]);
        assert_eq!(stats.skipped_files, 1);
        assert_eq!(stats.hash_missing, 1);
    }

    #[test]
    fn verify_skips_normal_file_with_matching_local_hash() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("config"), b"hello").unwrap();
        let source = vec![file_entry("config", 5, None, None)];
        let target = vec![file_entry("config", 5, Some("id-1"), Some(HELLO_SHA256))];
        let (actions, stats) =
            compute_diff_with_stats(&source, &target, verify_options(tmp.path().to_path_buf()));
        assert_eq!(action_kinds(&actions), vec!["skip"]);
        assert_eq!(stats.skipped_files, 1);
    }

    #[test]
    fn verify_replaces_normal_file_with_wrong_hash_or_missing_hash() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("config"), b"hello").unwrap();
        let source = vec![file_entry("config", 5, None, None)];
        let wrong = vec![file_entry("config", 5, Some("id-1"), Some(&"c".repeat(64)))];
        let (actions, _) =
            compute_diff_with_stats(&source, &wrong, verify_options(tmp.path().to_path_buf()));
        assert_eq!(action_kinds(&actions), vec!["upload"]);

        let missing = vec![file_entry("config", 5, Some("id-1"), None)];
        let (actions, _) =
            compute_diff_with_stats(&source, &missing, verify_options(tmp.path().to_path_buf()));
        assert_eq!(action_kinds(&actions), vec!["upload"]);
    }

    #[test]
    fn verify_replaces_on_size_difference() {
        let tmp = tempfile::tempdir().unwrap();
        let name = "a".repeat(64);
        let path = format!("data/aa/{name}");
        let source = vec![file_entry(&path, 11, None, None)];
        let target = vec![file_entry(&path, 10, Some("id-1"), Some(&name))];
        let (actions, _) =
            compute_diff_with_stats(&source, &target, verify_options(tmp.path().to_path_buf()));
        assert_eq!(action_kinds(&actions), vec!["upload"]);
    }

    #[test]
    fn verify_stats_only_count_files_not_directories() {
        let tmp = tempfile::tempdir().unwrap();
        let name = "a".repeat(64);
        let path = format!("data/aa/{name}");
        let source = vec![
            file_entry(&path, 10, None, None),
            dir_entry("data/aa", None),
            dir_entry("empty", None),
        ];
        let target = vec![
            file_entry(&path, 10, Some("id-1"), Some(&name)),
            file_entry("extra.txt", 1, Some("id-2"), None),
            dir_entry("data/aa", Some("id-dir")),
            dir_entry("extra-dir", Some("id-3")),
        ];
        let mut options = verify_options(tmp.path().to_path_buf());
        options.delete = true;
        let (_actions, stats) = compute_diff_with_stats(&source, &target, options);
        assert_eq!(stats.objects, 1, "objects 只应统计文件");
        assert_eq!(stats.planned_deleted_files, 1, "删除计划只应统计文件");
    }

    #[test]
    fn actions_carry_scan_file_ids() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("sub"), b"x").unwrap();
        // 本地文件在新目录 sub 下；云端扫描里有 sub 目录及其 fileId。
        let source = vec![file_entry("sub/new.txt", 1, None, None)];
        let target = vec![dir_entry("sub", Some("dir-sub"))];
        let options = SyncDiffOptions {
            direction: SyncDirection::LocalToCloud,
            delete: false,
            local_root: tmp.path().to_path_buf(),
            cloud_root: "/remote".to_string(),
            verify: false,
        };
        let (actions, _) = compute_diff_with_stats(&source, &target, options);
        let upload = actions
            .iter()
            .find(|action| matches!(action, SyncAction::Upload { .. }))
            .expect("should upload");
        match upload {
            SyncAction::Upload { parent_file_id, .. } => {
                assert_eq!(parent_file_id.as_deref(), Some("dir-sub"))
            }
            _ => unreachable!(),
        }
    }
}
