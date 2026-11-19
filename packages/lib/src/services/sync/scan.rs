use super::paths::{compile_exclude_patterns, is_excluded, is_recovery_name, path_and_ancestors};
use super::remote::SYNC_LIST_RETRY;
use crate::client::api::personal::client::PersonalClient;
use crate::client::api::personal::listing::ListPageOptions;
use crate::client::http::HttpClientWrapper;
use crate::config::{Config, PersonalConfig};
use crate::models::sync_item::{FileEntry, SyncDirection, SyncEntryKind};
use crate::protocol::is_non_root_file_id;
use crate::services::directory::directory_with_listing;
use crate::services::local_scan::scan_items;
use crate::utils::remote_path;
use anyhow::{Result, anyhow};
use futures_util::TryStreamExt;
use glob::Pattern;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::pin::pin;

#[derive(Debug, Clone)]
pub struct SyncScanOptions {
    pub recursive: bool,
    pub exclude: Vec<String>,
}

pub fn scan_local(root: &Path, options: SyncScanOptions) -> Result<Vec<FileEntry>> {
    let patterns = compile_exclude_patterns(&options.exclude)?;
    scan_items(root, options.recursive, &patterns)
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

#[derive(Default)]
pub struct CloudSyncScan {
    pub items: Vec<FileEntry>,
    directories: BTreeMap<String, String>,
    protected: BTreeSet<String>,
    files: BTreeMap<String, CloudSyncFile>,
    pub(super) host: String,
    pub(super) root: String,
}

#[derive(PartialEq, Eq)]
pub(in crate::services) struct CloudSyncFile {
    pub(super) id: Option<String>,
    pub(in crate::services) size: Option<u64>,
    pub(in crate::services) sha256: Option<String>,
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
        Self {
            id,
            size,
            sha256: item.content_sha256(),
            modified: item.modified_time().map(str::to_owned),
        }
    }
}

impl CloudSyncScan {
    pub(super) fn is_protected(&self, rel_path: &str) -> bool {
        self.protected.contains(rel_path)
    }

    pub(in crate::services) fn file(&self, rel_path: &str) -> Option<&CloudSyncFile> {
        self.files.get(rel_path)
    }

    pub(super) fn directory_id(&self, rel_path: &str) -> Option<&str> {
        self.directories.get(rel_path).map(String::as_str)
    }

    pub(in crate::services) fn expected_id(&self, rel_path: &str) -> Result<Option<&str>> {
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

    pub(in crate::services) async fn confirm_subtree(
        &self,
        listing: &PersonalClient<'_>,
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
    config: &Config,
    cloud_root: &str,
    options: &SyncScanOptions,
    direction: SyncDirection,
) -> Result<CloudSyncScan> {
    let config = PersonalConfig::new(config)?;
    let cloud_root = remote_path::normalize(cloud_root)?;
    let patterns = compile_exclude_patterns(&options.exclude)?;
    scan_personal_cloud(config, &cloud_root, options.recursive, &patterns, direction).await
}

pub(super) async fn scan_personal_cloud(
    config: PersonalConfig<&Config>,
    cloud_root: &str,
    recursive: bool,
    patterns: &[Pattern],
    direction: SyncDirection,
) -> Result<CloudSyncScan> {
    let client = HttpClientWrapper::new();
    let listing = PersonalClient::connect(config, &client, SYNC_LIST_RETRY).await?;
    let root_id = directory_with_listing(&listing, cloud_root).await?;
    let mut scanner = CloudScanner {
        scan: CloudSyncScan {
            directories: BTreeMap::from([(String::new(), root_id.clone())]),
            host: listing.host().to_string(),
            root: cloud_root.to_owned(),
            ..CloudSyncScan::default()
        },
        seen_objects: BTreeSet::from([root_id.clone()]),
        recursive,
        upload_target: direction == SyncDirection::LocalToCloud,
        patterns,
    };
    scanner.scan_directory(&listing, "", &root_id).await?;
    scanner
        .scan
        .items
        .sort_by(|left, right| left.rel_path.cmp(&right.rel_path));
    Ok(scanner.scan)
}

struct CloudScanner<'a> {
    scan: CloudSyncScan,
    seen_objects: BTreeSet<String>,
    recursive: bool,
    upload_target: bool,
    patterns: &'a [Pattern],
}

impl CloudScanner<'_> {
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
                if is_excluded(&rel_path, self.patterns)
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
                            // 普通的非递归扫描只会遇到顶层子目录。
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
