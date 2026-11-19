pub use super::sync_local::scan_local;
pub(crate) use super::sync_local::{LocalSyncScan, scan_local_target, scan_upload_source};
use crate::client::api::{HttpClientWrapper, ListPageOptions, ListingRetry, PersonalClient};
use crate::debug;
use crate::domain::{
    ChangeKind, FileEntry, SyncAction, SyncDirection, SyncEndpoint, SyncEntryKind, SyncTarget,
};
use crate::utils::path::rel_to_local;
use crate::utils::remote_file::{is_non_root_file_id, normalize_content_sha256};
pub use crate::utils::remote_path::{cloud_child_path, normalize_cloud_path};
use anyhow::{Result, anyhow};
pub use execution::{SyncOutcome, SyncPlan, SyncRequest};
use futures_util::TryStreamExt;
use glob::Pattern;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::pin::pin;

mod execution;

#[derive(Debug, Clone)]
pub struct SyncDiffOptions {
    pub direction: SyncDirection,
    pub delete: bool,
    pub local_root: PathBuf,
    pub cloud_root: String,
}

pub fn parse_sync_endpoint(value: &str) -> SyncEndpoint {
    if let Some(rest) = value.strip_prefix("cloud:") {
        return SyncEndpoint::Cloud(normalize_cloud_path(rest));
    }
    SyncEndpoint::Local(PathBuf::from(value))
}

pub fn sync_direction(src: &SyncEndpoint, dest: &SyncEndpoint) -> Result<SyncDirection> {
    match (src, dest) {
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

pub fn compute_diff(
    source: &[FileEntry],
    target: &[FileEntry],
    options: SyncDiffOptions,
) -> Vec<SyncAction> {
    compute_diff_inner(source, target, options, None, None, false)
}

fn compute_diff_inner(
    source: &[FileEntry],
    target: &[FileEntry],
    options: SyncDiffOptions,
    cloud_target: Option<&CloudSyncScan>,
    local_target: Option<&LocalSyncScan>,
    verify: bool,
) -> Vec<SyncAction> {
    debug!(
        "开始计算同步差异 source={} target={} direction={:?}",
        source.len(),
        target.len(),
        options.direction
    );
    let source_by_path: BTreeMap<&str, &FileEntry> = source
        .iter()
        .map(|item| (item.rel_path.as_str(), item))
        .collect();
    let target_by_path: BTreeMap<&str, &FileEntry> = target
        .iter()
        .map(|item| (item.rel_path.as_str(), item))
        .collect();

    let recovery_paths: BTreeSet<_> = if options.direction == SyncDirection::LocalToCloud {
        target_by_path
            .keys()
            .copied()
            .filter(|path| is_recovery_path(path))
            .flat_map(path_and_ancestors)
            .collect()
    } else {
        BTreeSet::new()
    };
    let mut paths = BTreeSet::new();
    paths.extend(source_by_path.keys().copied());
    paths.extend(target_by_path.keys().copied());

    let mut actions = Vec::new();

    for path in paths {
        let protected = recovery_paths.contains(path)
            || cloud_target.is_some_and(|scan| scan.protected.contains(path))
            || local_target.is_some_and(|scan| scan.protected.contains(path));
        if options.direction == SyncDirection::LocalToCloud && is_recovery_path(path) {
            if source_by_path.contains_key(path) {
                actions.push(SyncAction::Conflict {
                    rel_path: path.to_owned(),
                    reason: "名称保留给同步临时副本和备份",
                });
            }
            continue;
        }
        if protected
            && source_by_path
                .get(path)
                .is_some_and(|entry| entry.kind == SyncEntryKind::File)
        {
            actions.push(SyncAction::Conflict {
                rel_path: path.to_owned(),
                reason: "目标同名目录未完整纳入同步范围，无法替换",
            });
            continue;
        }
        match (source_by_path.get(path), target_by_path.get(path)) {
            (Some(src), Some(dst)) if src.kind != dst.kind => {
                if src.kind == SyncEntryKind::Directory || local_target.is_some() {
                    actions.push(SyncAction::Conflict {
                        rel_path: path.to_owned(),
                        reason: "存在同名的不同类型条目，无法安全替换",
                    });
                } else {
                    let directory_id = cloud_target.and_then(|scan| scan.directories.get(path));
                    actions.push(transfer_action(
                        src,
                        &options,
                        ChangeKind::New,
                        directory_id.cloned(),
                    ));
                }
            }
            (Some(src), Some(dst)) => {
                let change = change_kind(src, dst, options.direction).or_else(|| {
                    (verify && src.kind == SyncEntryKind::File).then_some(ChangeKind::Verify)
                });
                if let Some(change) = change {
                    actions.push(transfer_action(src, &options, change, None));
                } else {
                    actions.push(SyncAction::Skip {
                        rel_path: (*path).to_string(),
                    });
                }
            }
            (Some(src), None) if src.kind == SyncEntryKind::Directory => {
                actions.push(create_dir_action(src, &options));
            }
            (Some(src), None) => {
                actions.push(transfer_action(src, &options, ChangeKind::New, None))
            }
            (None, Some(dst))
                if options.delete && !protected && (!verify || dst.kind == SyncEntryKind::File) =>
            {
                actions.push(delete_action(dst, &options));
            }
            (None, Some(_dst)) => actions.push(SyncAction::Skip {
                rel_path: (*path).to_string(),
            }),
            (None, None) => {}
        }
    }

    // An upload may replace a cloud directory. Keep its descendants until the
    // replacement succeeds; recycling the old directory also removes its children.
    let blocked_paths: BTreeSet<_> = actions
        .iter()
        .filter_map(|action| match action {
            SyncAction::Conflict { rel_path, .. } => Some(rel_path.clone()),
            _ => None,
        })
        .collect();
    let replacement_paths: BTreeSet<_> = actions
        .iter()
        .filter_map(|action| match action {
            SyncAction::Upload { rel_path, .. }
                if target_by_path
                    .get(rel_path.as_str())
                    .is_some_and(|target| target.kind == SyncEntryKind::Directory) =>
            {
                Some(rel_path.clone())
            }
            _ => None,
        })
        .collect();
    actions.retain(|action| match action {
        SyncAction::Upload { rel_path, .. }
        | SyncAction::Download { rel_path, .. }
        | SyncAction::CreateDir { rel_path, .. } => {
            !path_and_ancestors(rel_path).any(|path| blocked_paths.contains(path))
        }
        SyncAction::Delete { rel_path, .. } => !path_and_ancestors(rel_path)
            .any(|path| blocked_paths.contains(path) || replacement_paths.contains(path)),
        _ => true,
    });

    actions.sort_by_key(|action| match action {
        SyncAction::CreateDir { rel_path, .. } => {
            (0, rel_path.matches('/').count(), rel_path.clone())
        }
        SyncAction::Upload { rel_path, .. }
        | SyncAction::Download { rel_path, .. }
        | SyncAction::Conflict { rel_path, .. } => {
            (1, rel_path.matches('/').count(), rel_path.clone())
        }
        SyncAction::Skip { rel_path } => (2, rel_path.matches('/').count(), rel_path.clone()),
        SyncAction::Delete { rel_path, .. } => (
            3,
            usize::MAX - rel_path.matches('/').count(),
            rel_path.clone(),
        ),
    });

    debug!("已生成同步计划 actions={}", actions.len());
    actions
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
    replace_directory_id: Option<String>,
) -> SyncAction {
    match options.direction {
        SyncDirection::LocalToCloud => SyncAction::Upload {
            rel_path: entry.rel_path.clone(),
            local_abs: rel_to_local(&options.local_root, &entry.rel_path),
            remote_abs: cloud_child_path(&options.cloud_root, &entry.rel_path),
            size: entry.size,
            change,
            replace_directory_id,
        },
        SyncDirection::CloudToLocal => SyncAction::Download {
            rel_path: entry.rel_path.clone(),
            remote_abs: cloud_child_path(&options.cloud_root, &entry.rel_path),
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
            target_abs: cloud_child_path(&options.cloud_root, &entry.rel_path),
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
            target_abs: cloud_child_path(&options.cloud_root, &entry.rel_path),
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
        SyncAction::Conflict { rel_path, reason } => {
            format!("{prefix}*conflict    {rel_path}：{reason}")
        }
    }
}

fn change_marker(change: ChangeKind) -> &'static str {
    match change {
        ChangeKind::New => ">f+++++++++",
        ChangeKind::SizeOrTime => ">f.st......",
        ChangeKind::Verify => "?f.sha256..",
    }
}

#[derive(Debug, Clone)]
pub struct SyncScanOptions {
    pub recursive: bool,
    pub exclude: Vec<String>,
}

pub(super) fn compile_exclude_patterns(exclude: &[String]) -> Result<Vec<Pattern>> {
    exclude
        .iter()
        .map(|pattern| {
            Pattern::new(pattern).map_err(|err| anyhow!("无效 exclude glob `{}`: {}", pattern, err))
        })
        .collect()
}

pub(super) fn is_excluded(rel_path: &str, patterns: &[Pattern]) -> bool {
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
    })
}

fn parse_cloud_mtime(item: &crate::models::PersonalFileItem) -> Option<i64> {
    let raw = item.modified_time()?;

    chrono::DateTime::parse_from_rfc3339(raw)
        .map(|time| time.timestamp())
        .ok()
}

/// sync 的请求级重试参数：4 次重试（首次 + 4），退避 2/4/8/16 秒。
///
/// 只作用于**读操作**（列表、下载、路径解析）；上传/建目录等写操作不重试。
pub(crate) const SYNC_RETRIES: u32 = 4;
pub(crate) const SYNC_BACKOFF_SECS: u64 = 2;

pub(super) const SYNC_LIST_RETRY: ListingRetry = ListingRetry::Transient {
    retries: SYNC_RETRIES,
    backoff_secs: SYNC_BACKOFF_SECS,
    timeout: None,
};

pub(crate) const SYNC_UPLOAD_PREFIX: &str = ".cloud139-upload-";
pub(crate) const UPLOAD_BACKUP_PREFIX: &str = ".cloud139-backup-";

fn is_recovery_name(name: &str) -> bool {
    name.starts_with(SYNC_UPLOAD_PREFIX) || name.starts_with(UPLOAD_BACKUP_PREFIX)
}

fn is_recovery_path(path: &str) -> bool {
    path.split('/').any(is_recovery_name)
}

pub(super) fn path_and_ancestors(path: &str) -> impl Iterator<Item = &str> {
    std::iter::successors(Some(path), |path| {
        path.rsplit_once('/').map(|(parent, _)| parent)
    })
}

#[derive(Default)]
pub struct CloudSyncScan {
    pub items: Vec<FileEntry>,
    directories: BTreeMap<String, String>,
    protected: BTreeSet<String>,
    pub(crate) files: BTreeMap<String, CloudSyncFile>,
    pub(crate) host: String,
    pub(crate) root: String,
}

#[derive(PartialEq, Eq)]
pub(crate) struct CloudSyncFile {
    pub(crate) id: Option<String>,
    pub(crate) size: Option<u64>,
    pub(crate) sha256: Option<String>,
    modified: Option<String>,
}

impl From<&crate::models::PersonalFileItem> for CloudSyncFile {
    fn from(item: &crate::models::PersonalFileItem) -> Self {
        let id = item
            .file_id
            .as_ref()
            .filter(|id| is_non_root_file_id(id))
            .cloned();
        let size = item.size.and_then(|size| u64::try_from(size).ok());
        let sha256 = normalize_content_sha256(
            item.content_hash.as_deref(),
            item.content_hash_algorithm.as_deref(),
        );
        Self {
            id,
            size,
            sha256,
            modified: item.modified_time().map(str::to_owned),
        }
    }
}

impl CloudSyncScan {
    pub(crate) fn compute_upload_diff(
        &self,
        source: &[FileEntry],
        options: SyncDiffOptions,
        verify: bool,
    ) -> Vec<SyncAction> {
        compute_diff_inner(source, &self.items, options, Some(self), None, verify)
    }

    pub(crate) fn compute_download_diff(
        &self,
        target: &LocalSyncScan,
        options: SyncDiffOptions,
    ) -> Vec<SyncAction> {
        compute_diff_inner(
            &self.items,
            &target.items,
            options,
            None,
            Some(target),
            false,
        )
    }

    pub(crate) fn directory_id(&self, rel_path: &str) -> Option<&str> {
        self.directories.get(rel_path).map(String::as_str)
    }

    pub(super) fn expected_id(&self, rel_path: &str) -> Result<Option<&str>> {
        use anyhow::Context;
        if let Some(file) = self.files.get(rel_path) {
            return Ok(Some(file.id.as_deref().context("扫描未返回有效文件 ID")?));
        }
        Ok(self.directory_id(rel_path))
    }

    pub(super) fn descendant_file_paths(&self, root_path: &str) -> impl Iterator<Item = &str> {
        let prefix = format!("{root_path}/");
        self.files
            .range(prefix.clone()..)
            .map(|(path, _)| path.as_str())
            .take_while(move |path| path.starts_with(&prefix))
    }

    pub(super) async fn confirm_subtree(
        &self,
        listing: &crate::client::api::PersonalClient<'_>,
        root_path: &str,
        root_id: &str,
    ) -> Result<()> {
        let prefix = format!("{root_path}/");
        let files = self.descendant_file_paths(root_path);
        let directories = self
            .directories
            .range(prefix.clone()..)
            .map(|(path, _)| path.as_str())
            .take_while(|path| path.starts_with(&prefix));
        let mut children = BTreeMap::<&str, BTreeSet<&str>>::new();
        for path in files.chain(directories) {
            let (parent, _) = path.rsplit_once('/').expect("subtree path has parent");
            children.entry(parent).or_default().insert(path);
        }
        let mut pending = vec![(root_path.to_owned(), root_id.to_owned())];
        while let Some((parent, parent_id)) = pending.pop() {
            let expected_paths = children.remove(parent.as_str()).unwrap_or_default();
            let current = listing.list_files(&parent_id).await?;
            anyhow::ensure!(
                current.len() == expected_paths.len(),
                "云端目录 {parent} 的内容已变更"
            );
            let mut seen = BTreeSet::new();
            for item in &current {
                let name = item
                    .name
                    .as_deref()
                    .ok_or_else(|| anyhow!("云端目录条目缺少名称"))?;
                let path = format!("{parent}/{name}");
                anyhow::ensure!(
                    expected_paths.contains(path.as_str()) && seen.insert(path.clone()),
                    "云端目录 {parent} 的内容已变更"
                );
                if item.is_folder() {
                    let id = self
                        .directory_id(&path)
                        .ok_or_else(|| anyhow!("云端目录 {path} 已变更"))?;
                    anyhow::ensure!(
                        item.file_id.as_deref() == Some(id),
                        "云端目录 {path} 已变更"
                    );
                    pending.push((path, id.to_owned()));
                } else {
                    anyhow::ensure!(
                        self.files.get(&path) == Some(&CloudSyncFile::from(item))
                            && self.expected_id(&path)?.is_some(),
                        "云端文件 {path} 已变更"
                    );
                }
            }
        }
        Ok(())
    }

    fn protect(&mut self, rel_path: &str) {
        self.protected
            .extend(path_and_ancestors(rel_path).map(str::to_owned));
    }
}

/// 扫描云端目录，按同步方向应用对应的扫描规则。
pub async fn scan_cloud(
    config: &crate::config::Config,
    cloud_root: &str,
    options: &SyncScanOptions,
    direction: SyncDirection,
) -> Result<CloudSyncScan> {
    config.require_personal()?;
    let cloud_root = super::directory_service::normalize(cloud_root)?;
    let client = HttpClientWrapper::new();
    let listing = PersonalClient::connect(config, &client, SYNC_LIST_RETRY).await?;
    let root_id = super::directory_service::directory_with_listing(&listing, &cloud_root).await?;
    let mut scanner = CloudScanner {
        scan: CloudSyncScan {
            directories: BTreeMap::from([(String::new(), root_id.clone())]),
            host: listing.host().to_string(),
            root: cloud_root,
            ..CloudSyncScan::default()
        },
        seen_objects: BTreeSet::from([root_id.clone()]),
        recursive: options.recursive,
        upload_target: direction == SyncDirection::LocalToCloud,
        patterns: compile_exclude_patterns(&options.exclude)?,
    };
    scanner.scan_directory(&listing, "", &root_id).await?;
    scanner
        .scan
        .items
        .sort_by(|left, right| left.rel_path.cmp(&right.rel_path));
    Ok(scanner.scan)
}

struct CloudScanner {
    scan: CloudSyncScan,
    seen_objects: BTreeSet<String>,
    recursive: bool,
    upload_target: bool,
    patterns: Vec<Pattern>,
}

impl CloudScanner {
    async fn scan_directory(
        &mut self,
        listing: &PersonalClient<'_>,
        rel_parent: &str,
        parent_file_id: &str,
    ) -> Result<()> {
        let mut pages = pin!(listing.pages(
            parent_file_id,
            ListPageOptions {
                thumbnails: None,
                ..ListPageOptions::default()
            }
        ));
        while let Some(items) = pages.try_next().await? {
            for item in items {
                let name = item
                    .name
                    .as_deref()
                    .ok_or_else(|| anyhow!("云端条目缺少名称"))?;
                super::directory_service::validate_name(name)?;
                let rel_path = if rel_parent.is_empty() {
                    name.to_owned()
                } else {
                    format!("{}/{}", rel_parent, name)
                };
                if is_excluded(&rel_path, &self.patterns)
                    || (self.upload_target && is_recovery_name(name))
                {
                    if self.upload_target {
                        self.scan.protect(&rel_path);
                    }
                    continue;
                }

                if item.is_folder() {
                    let child_id = item.file_id.as_deref().filter(|id| is_non_root_file_id(id));
                    if !self.upload_target && self.recursive && child_id.is_none() {
                        return Err(anyhow!("云端目录缺少有效 ID: {rel_path}"));
                    }
                    let Some(child_id) = child_id.filter(|_| self.recursive) else {
                        if self.upload_target {
                            // Ordinary non-recursive scans encounter only top-level folders.
                            self.scan.protect(&rel_path);
                        }
                        continue;
                    };
                    if let Some(previous) = self.scan.directories.get(&rel_path)
                        && previous == child_id
                    {
                        continue;
                    }
                    if self.scan.directories.contains_key(&rel_path)
                        || self.scan.files.contains_key(&rel_path)
                    {
                        return Err(anyhow!("云端扫描返回了冲突的路径: {rel_path}"));
                    }
                    if !self.seen_objects.insert(child_id.to_owned()) {
                        return Err(anyhow!("云端扫描返回了重复的目录 ID: {rel_path}"));
                    }
                    self.scan
                        .directories
                        .insert(rel_path.clone(), child_id.to_owned());
                    self.scan.items.push(FileEntry {
                        rel_path: rel_path.clone(),
                        size: 0,
                        mtime: parse_cloud_mtime(&item),
                        kind: SyncEntryKind::Directory,
                    });
                    Box::pin(self.scan_directory(listing, &rel_path, child_id)).await?;
                } else {
                    let entry = personal_item_to_file_entry(rel_parent, &item)?;
                    let file = CloudSyncFile::from(&item);
                    if let Some(previous) = self.scan.files.get(&rel_path) {
                        if file.id.is_some() && previous == &file {
                            continue;
                        }
                        return Err(anyhow!("云端扫描返回了冲突的路径: {rel_path}"));
                    }
                    if self.scan.directories.contains_key(&rel_path) {
                        return Err(anyhow!("云端扫描返回了冲突的路径: {rel_path}"));
                    }
                    if let Some(id) = &file.id
                        && !self.seen_objects.insert(id.to_owned())
                    {
                        return Err(anyhow!("云端扫描返回了重复的条目 ID: {rel_path}"));
                    }
                    self.scan.files.insert(rel_path, file);
                    self.scan.items.push(entry);
                }
            }
        }
        Ok(())
    }
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
