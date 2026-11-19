pub mod executor;
pub(super) mod paths;
pub(super) mod remote;
pub mod report;
mod verify;

use self::executor::{SyncExecuteOptions, execute_scanned_sync};
use self::paths::{compile_exclude_patterns, is_excluded, path_and_ancestors};
use self::report::SyncReport;
use self::verify::execute_verified_sync;
use super::local_scan::{LocalSyncScan, scan_items, scan_local_target, scan_upload_source};
use crate::client::api::personal::listing::ListPageOptions;
use crate::client::api::personal::{ListingRetry, PersonalClient};
use crate::client::http::HttpClientWrapper;
use crate::config::Config;
use crate::debug;
use crate::models::sync_item::{
    ChangeKind, FileEntry, SyncAction, SyncDirection, SyncEndpoint, SyncEntryKind, SyncSummary,
    SyncTarget,
};
use crate::progress::ProgressFactory;
use crate::utils::path::rel_to_local;
use crate::utils::remote_file::{is_non_root_file_id, normalize_content_sha256};
use crate::utils::remote_path;
use anyhow::{Result, anyhow, ensure};
use futures_util::TryStreamExt;
use glob::Pattern;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::pin::pin;
use std::time::Instant;

/// Validated options for a local-to-cloud or cloud-to-local sync.
pub struct SyncRequest {
    source: SyncEndpoint,
    destination: SyncEndpoint,
    direction: SyncDirection,
    scan: SyncScanOptions,
    execution: SyncExecuteOptions,
    delete: bool,
    verify: bool,
}

/// Scanned targets and the actions computed for a sync.
pub struct SyncPlan {
    config: Config,
    actions: Vec<SyncAction>,
    cloud: CloudSyncScan,
    local: Option<LocalSyncScan>,
    execution: SyncExecuteOptions,
    mode: SyncMode,
}

enum SyncMode {
    Ordinary(Option<SyncReport>),
    Verify { objects: usize, track_paths: bool },
}

/// The summary or detailed file report produced by a sync.
pub enum SyncOutcome {
    Summary(SyncSummary),
    Report(SyncReport),
}

impl SyncRequest {
    pub fn new(
        source: &str,
        destination: &str,
        scan: SyncScanOptions,
        execution: SyncExecuteOptions,
        delete: bool,
        verify: bool,
    ) -> Result<Self> {
        let source = parse_sync_endpoint(source);
        let destination = parse_sync_endpoint(destination);
        let direction = sync_direction(&source, &destination)?;
        if verify {
            ensure!(
                direction == SyncDirection::LocalToCloud,
                "--verify 要求目标路径以 cloud: 开头"
            );
            ensure!(execution.jobs == 1, "--verify 只允许 -j 1");
        }
        Ok(Self {
            source,
            destination,
            direction,
            scan,
            execution,
            delete,
            verify,
        })
    }

    pub async fn plan(self, mut config: Config, track_paths: bool) -> Result<SyncPlan> {
        let diff_options = |local_root, cloud_root| SyncDiffOptions {
            direction: self.direction,
            delete: self.delete,
            local_root,
            cloud_root,
        };
        let mut report =
            (track_paths && !self.verify).then(|| SyncReport::new(self.execution.dry_run, true));
        let (actions, cloud, local, mode) = match (self.source, self.destination) {
            (SyncEndpoint::Local(local_root), SyncEndpoint::Cloud(cloud_root)) => {
                let (local_root, source) = scan_upload_source(
                    local_root,
                    self.scan.recursive,
                    &self.scan.exclude,
                    self.delete,
                )?;
                let objects = source
                    .iter()
                    .filter(|entry| entry.kind == SyncEntryKind::File)
                    .count();
                ensure!(
                    !self.verify || !self.delete || objects > 0,
                    "--verify --delete 要求同步范围内至少有一个源文件"
                );
                let cloud = scan_cloud(&config, &cloud_root, &self.scan, self.direction).await?;
                crate::step!("正在计算同步差异...");
                let actions = cloud.compute_upload_diff(
                    &source,
                    diff_options(local_root, cloud.root.clone()),
                    self.verify,
                );
                if let Some(report) = &mut report {
                    report.count_source_files(&source, &actions);
                }
                let mode = if self.verify {
                    SyncMode::Verify {
                        objects,
                        track_paths,
                    }
                } else {
                    SyncMode::Ordinary(report)
                };
                (actions, cloud, None, mode)
            }
            (SyncEndpoint::Cloud(cloud_root), SyncEndpoint::Local(local_root)) => {
                let cloud = scan_cloud(&config, &cloud_root, &self.scan, self.direction).await?;
                let local =
                    scan_local_target(&local_root, self.scan.recursive, &self.scan.exclude)?;
                local.validate_destinations(&cloud.items, &local_root)?;
                crate::step!("正在计算同步差异...");
                let actions =
                    cloud.compute_download_diff(&local, diff_options(local_root, cloud_root));
                if let Some(report) = &mut report {
                    report.count_source_files(&cloud.items, &actions);
                }
                (actions, cloud, Some(local), SyncMode::Ordinary(report))
            }
            _ => anyhow::bail!("无效的同步方向"),
        };
        config.personal_cloud_host = Some(cloud.host.clone());
        Ok(SyncPlan {
            config,
            actions,
            cloud,
            local,
            execution: self.execution,
            mode,
        })
    }
}

impl SyncPlan {
    pub fn actions(&self) -> &[SyncAction] {
        &self.actions
    }

    pub async fn execute(self) -> Result<SyncOutcome> {
        self.execute_with_progress(None).await
    }

    pub async fn execute_with_progress(
        self,
        progress: Option<&ProgressFactory<'_>>,
    ) -> Result<SyncOutcome> {
        match self.mode {
            SyncMode::Verify {
                objects,
                track_paths,
            } => {
                let report = execute_verified_sync(
                    &self.config,
                    &self.cloud,
                    &self.actions,
                    objects,
                    self.execution,
                    track_paths,
                    progress,
                )
                .await;
                Ok(SyncOutcome::Report(report))
            }
            SyncMode::Ordinary(mut report) => {
                let started = Instant::now();
                let summary = execute_scanned_sync(
                    &self.config,
                    self.actions,
                    self.execution,
                    Some(&self.cloud),
                    self.local.as_ref(),
                    report.as_mut(),
                    progress,
                )
                .await?;
                if let Some(mut report) = report {
                    report.seconds = started.elapsed().as_secs_f64();
                    Ok(SyncOutcome::Report(report))
                } else {
                    Ok(SyncOutcome::Summary(summary))
                }
            }
        }
    }
}

impl SyncOutcome {
    pub fn failed(&self) -> usize {
        match self {
            Self::Summary(summary) => summary.failed,
            Self::Report(report) => report.failures.len(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SyncDiffOptions {
    pub direction: SyncDirection,
    pub delete: bool,
    pub local_root: PathBuf,
    pub cloud_root: String,
}

pub fn parse_sync_endpoint(value: &str) -> SyncEndpoint {
    if let Some(rest) = value.strip_prefix("cloud:") {
        return SyncEndpoint::Cloud(remote_path::normalize_cloud_path(rest));
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
            remote_abs: remote_path::cloud_child_path(&options.cloud_root, &entry.rel_path),
            size: entry.size,
            change,
            replace_directory_id,
        },
        SyncDirection::CloudToLocal => SyncAction::Download {
            rel_path: entry.rel_path.clone(),
            remote_abs: remote_path::cloud_child_path(&options.cloud_root, &entry.rel_path),
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
            target_abs: remote_path::cloud_child_path(&options.cloud_root, &entry.rel_path),
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
            target_abs: remote_path::cloud_child_path(&options.cloud_root, &entry.rel_path),
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

pub fn scan_local(root: &Path, options: SyncScanOptions) -> Result<Vec<FileEntry>> {
    scan_items(root, options.recursive, &options.exclude)
}

pub fn personal_item_to_file_entry(
    rel_parent: &str,
    item: &crate::models::list::PersonalFileItem,
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

fn parse_cloud_mtime(item: &crate::models::list::PersonalFileItem) -> Option<i64> {
    let raw = item.modified_time()?;

    chrono::DateTime::parse_from_rfc3339(raw)
        .map(|time| time.timestamp())
        .ok()
}

/// sync 的请求级重试参数：4 次重试（首次 + 4），退避 2/4/8/16 秒。
///
/// 只作用于**读操作**（列表、下载、路径解析）；上传/建目录等写操作不重试。
const SYNC_RETRIES: u32 = 4;
const SYNC_BACKOFF_SECS: u64 = 2;

pub(super) const SYNC_LIST_RETRY: ListingRetry = ListingRetry::Transient {
    retries: SYNC_RETRIES,
    backoff_secs: SYNC_BACKOFF_SECS,
    timeout: None,
};

pub(super) const SYNC_UPLOAD_PREFIX: &str = ".cloud139-upload-";
pub(super) const UPLOAD_BACKUP_PREFIX: &str = ".cloud139-backup-";

fn is_recovery_name(name: &str) -> bool {
    name.starts_with(SYNC_UPLOAD_PREFIX) || name.starts_with(UPLOAD_BACKUP_PREFIX)
}

fn is_recovery_path(path: &str) -> bool {
    path.split('/').any(is_recovery_name)
}

#[derive(Default)]
pub struct CloudSyncScan {
    pub items: Vec<FileEntry>,
    directories: BTreeMap<String, String>,
    protected: BTreeSet<String>,
    pub(super) files: BTreeMap<String, CloudSyncFile>,
    host: String,
    root: String,
}

#[derive(PartialEq, Eq)]
pub(super) struct CloudSyncFile {
    id: Option<String>,
    pub(super) size: Option<u64>,
    pub(super) sha256: Option<String>,
    modified: Option<String>,
}

impl From<&crate::models::list::PersonalFileItem> for CloudSyncFile {
    fn from(item: &crate::models::list::PersonalFileItem) -> Self {
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
    fn compute_upload_diff(
        &self,
        source: &[FileEntry],
        options: SyncDiffOptions,
        verify: bool,
    ) -> Vec<SyncAction> {
        compute_diff_inner(source, &self.items, options, Some(self), None, verify)
    }

    fn compute_download_diff(
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

    fn directory_id(&self, rel_path: &str) -> Option<&str> {
        self.directories.get(rel_path).map(String::as_str)
    }

    pub(super) fn expected_id(&self, rel_path: &str) -> Result<Option<&str>> {
        use anyhow::Context;
        if let Some(file) = self.files.get(rel_path) {
            return Ok(Some(file.id.as_deref().context("扫描未返回有效文件 ID")?));
        }
        Ok(self.directory_id(rel_path))
    }

    fn descendant_file_paths(&self, root_path: &str) -> impl Iterator<Item = &str> {
        let prefix = format!("{root_path}/");
        self.files
            .range(prefix.clone()..)
            .map(|(path, _)| path.as_str())
            .take_while(move |path| path.starts_with(&prefix))
    }

    pub(super) async fn confirm_subtree(
        &self,
        listing: &crate::client::api::personal::PersonalClient<'_>,
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
    let cloud_root = remote_path::normalize(cloud_root)?;
    let client = HttpClientWrapper::new();
    let listing = PersonalClient::connect(config, &client, SYNC_LIST_RETRY).await?;
    let root_id = super::directory::directory_with_listing(&listing, &cloud_root).await?;
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
                remote_path::validate_name(name)?;
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
