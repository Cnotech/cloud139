use crate::client::{ClientError, api};
use crate::config::Config;
use crate::domain::{ChangeKind, SyncAction, SyncTarget};
use crate::services::sync_executor::{SyncExecuteOptions, make_action_progress};
use crate::services::sync_service::{
    CloudSyncFile, CloudSyncScan, SYNC_BACKOFF_SECS, SYNC_RETRIES, cloud_child_path,
    confirm_cloud_directory,
};
use crate::services::upload::personal::{
    SyncUploadPlan, SyncUploadVerification, upload_sync_to_parent,
};
use crate::utils::retry::{RetryError, with_retries};
use anyhow::{Context, Result, anyhow, ensure};
use indicatif::MultiProgress;
use serde::Serialize;
use std::collections::{BTreeMap, HashSet};
use std::time::Instant;

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SyncVerifyReport {
    pub(crate) objects: usize,
    pub(crate) uploaded: usize,
    pub(crate) skipped: usize,
    pub(crate) deleted: usize,
    pub(crate) hash_missing: usize,
    pub(crate) seconds: f64,
    pub(crate) failures: Vec<SyncFailure>,
}

#[derive(Serialize)]
pub(crate) struct SyncFailure {
    path: String,
    error: String,
    retries: u32,
}

impl SyncVerifyReport {
    fn fail(&mut self, path: &str, error: anyhow::Error) {
        let message = format!("{error:#}");
        crate::error!("同步失败: {path}: {message}");
        let retries = error
            .downcast_ref::<RetryError>()
            .map_or(0, |error| error.attempts);
        self.failures.push(SyncFailure {
            path: path.into(),
            error: message,
            retries,
        });
    }
}

pub(super) async fn remote_sha256(
    config: &Config,
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
        super::download_service::personal_content_sha256(config, host, id, expected_size)
            .await
            .map_err(Into::into)
    })
    .await
    .context("远端未提供可比的 SHA-256，读取内容校验失败")
}

pub(crate) async fn execute_verified_sync(
    config: &Config,
    cloud: &CloudSyncScan,
    cloud_root: &str,
    actions: &[SyncAction],
    objects: usize,
    options: SyncExecuteOptions,
) -> SyncVerifyReport {
    let started = Instant::now();
    let mut report = SyncVerifyReport {
        objects,
        ..Default::default()
    };
    let mut deletes = Vec::new();
    let mut retained_files = BTreeMap::new();
    let mut sync = VerifiedSync {
        config,
        cloud,
        cloud_root,
        directory_ids: BTreeMap::new(),
        http_client: api::HttpClientWrapper::new(),
    };
    if !options.dry_run
        && let Err(error) = sync.confirm_root().await
    {
        report.fail(cloud_root, error);
        report.seconds = started.elapsed().as_secs_f64();
        return report;
    }
    let progress = MultiProgress::new();
    for action in actions {
        match action {
            SyncAction::Conflict { rel_path, reason } => report.fail(rel_path, anyhow!("{reason}")),
            SyncAction::Upload { change, .. } if options.dry_run => {
                report.uploaded += usize::from(*change != ChangeKind::Verify);
            }
            SyncAction::Upload { rel_path, .. } => {
                let bar = make_action_progress(&progress, action, options.progress);
                let mut hash_missing = false;
                let result = sync
                    .upload_or_verify(action, bar.clone(), &mut hash_missing)
                    .await;
                if let Some(bar) = bar {
                    bar.finish_and_clear();
                }
                report.hash_missing += usize::from(hash_missing);
                match result {
                    Ok((id, uploaded)) => {
                        report.uploaded += usize::from(uploaded);
                        report.skipped += usize::from(!uploaded);
                        retained_files.insert(rel_path.as_str(), id);
                    }
                    Err(error) => report.fail(rel_path, error),
                }
            }
            SyncAction::CreateDir {
                rel_path,
                target: SyncTarget::Cloud,
                ..
            } if !options.dry_run => {
                if let Err(error) = sync.ensure_directory(rel_path).await {
                    report.fail(rel_path, error);
                }
            }
            SyncAction::Delete {
                rel_path,
                target: SyncTarget::Cloud,
                ..
            } => {
                deletes.push(rel_path.as_str());
            }
            _ => {}
        }
    }

    if report.failures.is_empty() && !deletes.is_empty() {
        if options.dry_run {
            report.deleted = deletes.len();
        } else if let Err((path, error)) = sync
            .delete_verified_extras(&retained_files, &deletes, &mut report)
            .await
        {
            report.fail(path, error);
        }
    }
    report.seconds = started.elapsed().as_secs_f64();
    report
}

struct VerifiedSync<'a> {
    config: &'a Config,
    cloud: &'a CloudSyncScan,
    cloud_root: &'a str,
    directory_ids: BTreeMap<String, String>,
    http_client: api::HttpClientWrapper,
}

impl VerifiedSync<'_> {
    async fn confirm_root(&self) -> Result<()> {
        let root = self
            .cloud
            .directory_id("")
            .context("扫描缺少云端根目录 ID")?;
        confirm_cloud_directory(self.config, self.cloud_root, root).await
    }

    fn listing(&self) -> api::PersonalListing<'_> {
        api::PersonalListing {
            config: self.config,
            host: &self.cloud.host,
            http_client: &self.http_client,
            retry_transient: true,
        }
    }

    async fn ensure_directory(&mut self, rel_path: &str) -> Result<String> {
        let mut id = self
            .cloud
            .directory_id("")
            .context("扫描缺少云端根目录 ID")?
            .to_owned();
        let mut path = String::new();
        for part in rel_path.split('/').filter(|part| !part.is_empty()) {
            if !path.is_empty() {
                path.push('/');
            }
            path.push_str(part);
            if let Some(existing) = self.directory_ids.get(&path) {
                id.clone_from(existing);
                continue;
            }
            let listing = self.listing();
            let child_id = if let Some(expected) = self.cloud.directory_id(&path) {
                let item = listing.find_child(&id, part).await?;
                ensure!(
                    item.is_some_and(
                        |item| item.is_folder() && item.file_id.as_deref() == Some(expected)
                    ),
                    "目标目录 {path} 在扫描后发生变化"
                );
                expected.to_owned()
            } else {
                let parent = cloud_child_path(self.cloud_root, parent_and_name(&path).0);
                confirm_cloud_directory(self.config, &parent, &id).await?;
                super::directory_service::ensure_personal_child_dir(
                    &listing, &id, part, true, false,
                )
                .await?
            };
            self.directory_ids.insert(path.clone(), child_id.clone());
            id = child_id;
        }
        Ok(id)
    }

    async fn upload_or_verify(
        &mut self,
        action: &SyncAction,
        pb: Option<indicatif::ProgressBar>,
        hash_missing: &mut bool,
    ) -> Result<(String, bool)> {
        let SyncAction::Upload {
            rel_path,
            local_abs,
            change,
            replace_directory_id,
            ..
        } = action
        else {
            unreachable!("only upload actions are verified");
        };
        let size = super::upload_service::checked_upload_size(local_abs)? as u64;
        let (parent, name) = parent_and_name(rel_path);
        let parent_id = self.ensure_directory(parent).await?;
        let previous = self.cloud.files.get(rel_path);
        let existing_id = if let Some(file) = previous {
            Some(
                file.id
                    .as_deref()
                    .context("原文件缺少 ID，无法验证或替换")?,
            )
        } else {
            replace_directory_id.as_deref()
        };

        if *change == ChangeKind::Verify
            && let Some(file) = previous
            && file.size.is_none_or(|remote_size| remote_size == size)
        {
            let path = local_abs.clone();
            let local_hash =
                tokio::task::spawn_blocking(move || crate::utils::crypto::calc_file_sha256(&path))
                    .await??;
            let remote_hash =
                remote_sha256(self.config, &self.cloud.host, file, file.size, hash_missing).await?;
            if local_hash == remote_hash {
                return Ok((existing_id.context("原文件缺少 ID")?.to_owned(), false));
            }
        }

        let id = upload_sync_to_parent(
            self.config,
            local_abs,
            SyncUploadPlan {
                host: &self.cloud.host,
                parent_id: &parent_id,
                remote_parent: &cloud_child_path(self.cloud_root, parent),
                file_name: name,
                replace_directory_id: replace_directory_id.as_deref(),
                verification: Some(SyncUploadVerification {
                    existing_id,
                    hash_missing,
                }),
            },
            pb,
        )
        .await?;
        Ok((id, true))
    }

    async fn delete_verified_extras<'a>(
        &self,
        retained: &BTreeMap<&'a str, String>,
        deletes: &[&'a str],
        report: &mut SyncVerifyReport,
    ) -> std::result::Result<(), (&'a str, anyhow::Error)> {
        let mut by_parent = BTreeMap::<&str, Vec<(&str, &str)>>::new();
        for (&path, id) in retained {
            by_parent
                .entry(parent_and_name(path).0)
                .or_default()
                .push((path, id));
        }
        let retained_ids: HashSet<_> = retained.values().map(String::as_str).collect();
        let mut deleted_ids = HashSet::new();
        let mut delete_ids = Vec::with_capacity(deletes.len());
        for &path in deletes {
            let id = self
                .cloud
                .files
                .get(path)
                .and_then(|file| file.id.as_deref())
                .ok_or_else(|| (path, anyhow!("计划删除的文件缺少扫描 ID")))?;
            if retained_ids.contains(id) || !deleted_ids.insert(id) {
                return Err((path, anyhow!("计划删除的文件 ID 重复或与保留文件相同")));
            }
            by_parent
                .entry(parent_and_name(path).0)
                .or_default()
                .push((path, id));
            delete_ids.push((path, id));
        }

        // Recheck every parent and file before submitting any trash request.
        for (parent, files) in by_parent {
            let path = files[0].0;
            let expected_id = self
                .directory_ids
                .get(parent)
                .map(String::as_str)
                .or_else(|| self.cloud.directory_id(parent))
                .ok_or_else(|| (path, anyhow!("删除前复核缺少云端目录 ID")))?;
            let remote = cloud_child_path(self.cloud_root, parent);
            confirm_cloud_directory(self.config, &remote, expected_id)
                .await
                .map_err(|error| (path, error))?;
            let current = self
                .listing()
                .list_files(expected_id)
                .await
                .map_err(|error| (path, error.into()))?;
            let mut by_name = BTreeMap::new();
            for item in &current {
                if let Some(name) = item.name.as_deref()
                    && by_name.insert(name, item).is_some()
                {
                    return Err((path, anyhow!("删除前复核发现云端目录有重名条目")));
                }
            }
            for (path, expected_id) in files {
                let name = parent_and_name(path).1;
                if !by_name.get(name).is_some_and(|item| {
                    !item.is_folder() && item.file_id.as_deref() == Some(expected_id)
                }) {
                    return Err((path, anyhow!("删除前复核发现云端文件已变更")));
                }
            }
        }

        for (path, id) in delete_ids {
            // Another client can move an object after the grouped preflight.
            let remote = cloud_child_path(self.cloud_root, path);
            let current = with_retries(SYNC_RETRIES, SYNC_BACKOFF_SECS, || async {
                super::directory_service::resolve(self.config, &remote)
                    .await
                    .map_err(Into::into)
            })
            .await
            .map_err(|error| (path, error))?;
            if current.folder || current.id != id {
                return Err((path, anyhow!("提交删除前发现云端文件已变更")));
            }
            super::delete_service::trash_personal_file_id(self.config, &self.cloud.host, id)
                .await
                .map_err(|error: ClientError| (path, error.into()))?;
            report.deleted += 1;
        }
        Ok(())
    }
}

fn parent_and_name(path: &str) -> (&str, &str) {
    path.rsplit_once('/').unwrap_or(("", path))
}
