use super::scan::{CloudSyncFile, CloudSyncScan};
use crate::client::api::personal::client::{ListingRetry, PersonalClient};
use crate::client::api::personal::listing::PathLookupCache;
use crate::client::http::HttpClientWrapper;
use crate::config::{Config, personal::PersonalConfig};
use crate::services::{directory as dirs, download};
use crate::utils::{
    remote_path::{self, cloud_child_path},
    retry::with_retries,
};
use anyhow::{Context, Result, ensure};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{Mutex, OnceCell};

/// 同步读请求最多重试 4 次，退避 2/4/8/16 秒。
///
/// 用于列表、下载和路径解析。
pub(super) const SYNC_RETRIES: u32 = 4;
pub(super) const SYNC_BACKOFF_SECS: u64 = 2;

pub(super) const SYNC_LIST_RETRY: ListingRetry = ListingRetry::Transient {
    retries: SYNC_RETRIES,
    backoff_secs: SYNC_BACKOFF_SECS,
    timeout: None,
};

pub(super) async fn remote_sha256(
    config: PersonalConfig<&Config>,
    host: &str,
    remote: &CloudSyncFile,
    expected_size: Option<u64>,
    hash_missing: &mut bool,
) -> Result<String> {
    if let Some(hash) = &remote.sha256 {
        return Ok(hash.clone());
    }
    *hash_missing = true;
    let id = remote
        .id
        .as_deref()
        .context("服务端未返回文件 ID，无法读取远端内容")?;
    with_retries(SYNC_RETRIES, SYNC_BACKOFF_SECS, || async {
        download::personal_content_sha256(config, host, id, expected_size)
            .await
            .map_err(Into::into)
    })
    .await
    .context("远端未提供可比的 SHA-256，读取内容校验失败")
}

pub(super) struct SyncCloud<'a> {
    config: PersonalConfig<&'a Config>,
    scan: Option<&'a CloudSyncScan>,
    client: HttpClientWrapper,
    host: OnceCell<String>,
    directories: Mutex<HashMap<String, Arc<OnceCell<String>>>>,
    confirmations: Mutex<HashMap<String, Arc<OnceCell<()>>>>,
    listings: Mutex<PathLookupCache>,
    root_confirmed: OnceCell<()>,
}

impl<'a> SyncCloud<'a> {
    pub(super) fn new(config: PersonalConfig<&'a Config>, scan: Option<&'a CloudSyncScan>) -> Self {
        Self {
            config,
            scan,
            client: HttpClientWrapper::new(),
            host: OnceCell::new(),
            directories: Mutex::default(),
            confirmations: Mutex::default(),
            listings: Mutex::default(),
            root_confirmed: OnceCell::new(),
        }
    }

    pub(super) async fn listing(&self) -> Result<PersonalClient<'_>> {
        let host = self
            .host
            .get_or_try_init(|| async {
                if let Some(scan) = self.scan {
                    return Ok(scan.host.clone());
                }
                self.client.resolve_personal_cloud_host(self.config).await
            })
            .await?;
        Ok(PersonalClient::with_host(
            self.config,
            host.as_str(),
            &self.client,
            ListingRetry::DIRECTORY,
        ))
    }

    pub(super) async fn ensure_directory(&self, absolute: &str) -> Result<String> {
        let normalized = remote_path::normalize(absolute)?;
        let root = self.scan.map_or("/", |scan| scan.root.as_str());
        let relative = if normalized == root {
            ""
        } else {
            normalized
                .strip_prefix(root.trim_end_matches('/'))
                .and_then(|suffix| suffix.strip_prefix('/'))
                .context("云端目录位于同步根目录之外")?
        };
        let mut path = root.to_owned();
        let listing = self.listing().await?;
        let root_id = self.directory_cell(root).await;
        let mut id = root_id
            .get_or_try_init(|| async {
                let id = dirs::directory_with_listing(&listing, root).await?;
                if let Some(scan) = self.scan {
                    ensure!(
                        scan.directory_id("") == Some(id.as_str()),
                        "云端根目录 ID 已变更"
                    );
                }
                Ok::<_, anyhow::Error>(id)
            })
            .await?
            .clone();

        let mut ancestors = std::collections::HashSet::from([id.clone()]);
        let mut rel_path = String::new();
        for name in relative.split('/').filter(|name| !name.is_empty()) {
            let parent_path = path;
            path = cloud_child_path(&parent_path, name);
            if !rel_path.is_empty() {
                rel_path.push('/');
            }
            rel_path.push_str(name);
            let directory = self.directory_cell(&path).await;
            let child = directory
                .get_or_try_init(|| async {
                    if let Some(expected) = self.scan.and_then(|scan| scan.directory_id(&rel_path))
                    {
                        let current = listing.find_child(&id, name).await?;
                        ensure!(
                            current.is_some_and(|item| item.is_folder()
                                && item.file_id.as_deref() == Some(expected)),
                            "云端目录 {path} 在扫描后发生变化"
                        );
                        return Ok(expected.to_owned());
                    }
                    if let Some(scan) = self.scan {
                        ensure!(
                            scan.file(&rel_path).is_none(),
                            "云端同名文件阻止创建目录 {path}"
                        );
                    }
                    self.confirm_directory(&parent_path, &id).await?;
                    Ok::<_, anyhow::Error>(
                        dirs::ensure_personal_child_dir(&listing, &id, name, true, false).await?,
                    )
                })
                .await?;
            ensure!(ancestors.insert(child.clone()), "云端目录 ID 重复: {path}");
            id.clone_from(child);
        }
        Ok(id)
    }

    async fn directory_cell(&self, path: &str) -> Arc<OnceCell<String>> {
        self.directories
            .lock()
            .await
            .entry(path.to_owned())
            .or_default()
            .clone()
    }

    pub(super) async fn confirm_directory(&self, path: &str, id: &str) -> Result<()> {
        let listing = self.listing().await?;
        dirs::confirm_directory(&listing, path, id).await
    }

    /// 用扫描记录的目录 ID 沿路径逐级核对，并按父目录复用列表分页。
    ///
    /// 扫描已记录每一级目录的 ID；从同步根向下逐级 `find_child` 即可恢复完整路径语义，
    /// 分页结果在一轮同步内复用，请求数不随路径深度或目录数量线性增长。
    ///
    /// 只用于只读预检。写操作（创建临时文件、回收）前必须改用 `confirm_directory`
    /// 做 fresh 的绝对路径确认：缓存只能证明子项未变，不能证明父目录仍在原路径。
    pub(super) async fn confirm_scanned_directory(&self, path: &str, id: &str) -> Result<()> {
        let Some(scan) = self.scan else {
            return self.confirm_directory(path, id).await;
        };
        // 同步根是写入前最值得复核的一级，每次都完整解析，不走缓存。
        if remote_path::normalize(path)? == scan.root {
            return self.confirm_directory(path, id).await;
        }
        let key = format!("{path}\u{0}{id}");
        let cell = self
            .confirmations
            .lock()
            .await
            .entry(key)
            .or_default()
            .clone();
        cell.get_or_try_init(|| async {
            self.confirm_scanned_directory_uncached(scan, path, id)
                .await
        })
        .await?;
        Ok(())
    }

    async fn confirm_scanned_directory_uncached(
        &self,
        scan: &CloudSyncScan,
        path: &str,
        id: &str,
    ) -> Result<()> {
        let normalized = remote_path::normalize(path)?;
        let root = scan.root.as_str();
        let chain = scanned_directory_chain(scan, path, id);
        if normalized != root && chain.is_none() {
            // 本轮新建目录或不在同步范围内：退回完整路径解析。
            let listing = self.listing().await?;
            return dirs::confirm_directory(&listing, path, id).await;
        }
        let listing = self.listing().await?;
        let mut cache = self.listings.lock().await;
        let mut lookup = cache.lookup(&listing);
        self.confirm_sync_root(&listing, scan).await?;
        let Some(chain) = chain else {
            ensure!(
                scan.directory_id("") == Some(id),
                "云端根目录 {path} 的 ID 已变更"
            );
            return Ok(());
        };
        let mut parent_id = scan.directory_id("").context("同步根目录缺少扫描 ID")?;
        for (name, child_id) in chain {
            let current = lookup.find_child(parent_id, &name).await?;
            ensure!(
                current.is_some_and(
                    |item| item.is_folder() && item.file_id.as_deref() == Some(child_id)
                ),
                "云端目录 {path} 在扫描后发生变化"
            );
            parent_id = child_id;
        }
        ensure!(parent_id == id, "云端目录 {path} 的 ID 已变更");
        Ok(())
    }

    /// 同步根在每轮第一次写入前从云端根完整解析一次，后续复用。
    async fn confirm_sync_root(
        &self,
        listing: &PersonalClient<'_>,
        scan: &CloudSyncScan,
    ) -> Result<()> {
        if self.root_confirmed.get().is_some() {
            return Ok(());
        }
        if scan.root == "/" {
            // 云端根目录没有父目录，也不可被改名。
            let _ = self.root_confirmed.set(());
            return Ok(());
        }
        let root_id = scan.directory_id("").context("同步根目录缺少扫描 ID")?;
        dirs::confirm_directory(listing, scan.root.as_str(), root_id).await?;
        let _ = self.root_confirmed.set(());
        Ok(())
    }

    pub(super) async fn delete_extra(&self, rel_path: &str) -> Result<()> {
        let scan = self.scan.context("删除云端条目需要扫描记录")?;
        let path = cloud_child_path(&scan.root, rel_path);
        let (parent, name) = rel_path.rsplit_once('/').unwrap_or(("", rel_path));
        let parent_id = scan
            .directory_id(parent)
            .context("待删除条目的父目录缺少扫描 ID")?;
        let expected_id = scan
            .expected_id(rel_path)?
            .context("待删除条目缺少扫描 ID")?;
        let listing = self.listing().await?;
        let folder = scan.directory_id(rel_path).is_some();
        if folder {
            ensure!(
                listing.list_files(expected_id).await?.is_empty(),
                "云端目录 {path} 仍有内容，保留目录"
            );
        }
        // 回收是不可逆写操作：必须 fresh 复核绝对路径。按旧 ID 重列只能证明
        // 子项未变，不能证明父目录仍在同步根下（被改名移出时会删到范围外对象）。
        self.confirm_directory(&cloud_child_path(&scan.root, parent), parent_id)
            .await?;
        let siblings = listing.list_files(parent_id).await?;
        let mut matching = siblings
            .iter()
            .filter(|item| item.name.as_deref() == Some(name));
        let current = matching.next().filter(|item| {
            item.file_id.as_deref() == Some(expected_id) && item.is_folder() == folder
        });
        ensure!(
            current.is_some()
                && matching.next().is_none()
                && (folder || current.map(CloudSyncFile::from).as_ref() == scan.file(rel_path)),
            "提交删除前发现云端条目 {path} 已变更"
        );
        listing.trash(expected_id).await?;
        Ok(())
    }
}

/// 返回从同步根到目标的逐级（名称，扫描 ID）链；不在扫描范围内时返回 None。
fn scanned_directory_chain<'a>(
    scan: &'a CloudSyncScan,
    path: &str,
    expected_id: &str,
) -> Option<Vec<(String, &'a str)>> {
    let root = scan.root.as_str();
    let normalized = remote_path::normalize(path).ok()?;
    let relative = normalized
        .strip_prefix(root.trim_end_matches('/'))?
        .strip_prefix('/')?;
    if relative.is_empty() {
        return None;
    }
    let mut chain = Vec::new();
    let mut prefix = String::new();
    for name in relative.split('/').filter(|name| !name.is_empty()) {
        if !prefix.is_empty() {
            prefix.push('/');
        }
        prefix.push_str(name);
        let id = scan.directory_id(&prefix)?;
        chain.push((name.to_owned(), id));
    }
    if chain.last().map(|(_, id)| *id) != Some(expected_id) {
        return None;
    }
    Some(chain)
}
