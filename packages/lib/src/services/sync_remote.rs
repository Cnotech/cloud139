use super::{
    directory_service as dirs,
    sync_service::{CloudSyncFile, CloudSyncScan},
};
use crate::utils::remote_path::cloud_child_path;
use crate::{client::api, config::Config};
use anyhow::{Context, Result, ensure};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{Mutex, OnceCell};

pub(super) struct SyncCloud<'a> {
    config: &'a Config,
    scan: Option<&'a CloudSyncScan>,
    client: api::HttpClientWrapper,
    host: OnceCell<String>,
    directories: Mutex<HashMap<String, Arc<OnceCell<String>>>>,
}

impl<'a> SyncCloud<'a> {
    pub(super) fn new(config: &'a Config, scan: Option<&'a CloudSyncScan>) -> Self {
        Self {
            config,
            scan,
            client: api::HttpClientWrapper::new(),
            host: OnceCell::new(),
            directories: Mutex::default(),
        }
    }

    pub(super) async fn listing(&self) -> Result<api::PersonalClient<'_>> {
        let host = self
            .host
            .get_or_try_init(|| async {
                if let Some(scan) = self.scan {
                    return Ok(scan.host.clone());
                }
                self.client
                    .get_personal_cloud_host(&mut self.config.clone())
                    .await
            })
            .await?;
        Ok(api::PersonalClient::with_host(
            self.config,
            host.as_str(),
            &self.client,
            api::ListingRetry::DIRECTORY,
        ))
    }

    pub(super) async fn ensure_directory(&self, absolute: &str) -> Result<String> {
        let normalized = dirs::normalize(absolute)?;
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
                            !scan.files.contains_key(&rel_path),
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

    pub(super) async fn current_file(
        &self,
        parent: &str,
        name: &str,
        id: &str,
    ) -> Result<CloudSyncFile> {
        let listing = self.listing().await?;
        let files = listing.list_files(parent).await?;
        let mut matching = files
            .iter()
            .filter(|item| item.name.as_deref() == Some(name));
        let file = matching
            .next()
            .filter(|item| !item.is_folder() && item.file_id.as_deref() == Some(id))
            .context("云端文件在扫描后发生变化")?;
        ensure!(matching.next().is_none(), "云端存在多个同名文件: {name}");
        Ok(CloudSyncFile::from(file))
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
                && (folder
                    || current.map(CloudSyncFile::from).as_ref() == scan.files.get(rel_path)),
            "提交删除前发现云端条目 {path} 已变更"
        );
        listing.trash(expected_id).await?;
        Ok(())
    }
}
