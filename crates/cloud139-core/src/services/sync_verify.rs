use super::progress::ProgressFactory;
use super::sync_remote::SyncCloud;
use super::sync_report::SyncReport;
use crate::config::Config;
use crate::domain::{ChangeKind, SyncAction, SyncTarget};
use crate::services::sync_executor::{SyncExecuteOptions, make_action_progress};
use crate::services::sync_service::{
    CloudSyncFile, CloudSyncScan, SYNC_BACKOFF_SECS, SYNC_RETRIES, cloud_child_path,
};
use crate::services::upload::personal::{SyncUploadPlan, upload_sync_to_parent};
use crate::utils::retry::with_retries;
use anyhow::{Context, Result, anyhow, ensure};
use std::collections::{BTreeMap, HashSet};
use std::time::Instant;

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
    actions: &[SyncAction],
    objects: usize,
    options: SyncExecuteOptions,
    track_paths: bool,
    progress: Option<&ProgressFactory<'_>>,
) -> SyncReport {
    let cloud_root = &cloud.root;
    let started = Instant::now();
    let mut report = SyncReport::new(options.dry_run, track_paths);
    report.objects = objects;
    let mut deletes = Vec::new();
    let mut retained_files = BTreeMap::new();
    let sync = VerifiedSync {
        config,
        cloud,
        cloud_root,
        remote: SyncCloud::new(config, Some(cloud)),
    };
    if !options.dry_run
        && let Err(error) = sync.confirm_root().await
    {
        report.log_failure(cloud_root, error);
        report.seconds = started.elapsed().as_secs_f64();
        return report;
    }
    for action in actions {
        match action {
            SyncAction::Conflict { rel_path, reason } => {
                report.log_failure(rel_path, anyhow!("{reason}"));
            }
            SyncAction::Upload { .. } if options.dry_run => {
                report.record_file_action(action, Some(cloud));
            }
            SyncAction::Upload { rel_path, .. } => {
                let bar = make_action_progress(progress, action);
                let mut hash_missing = false;
                let mut file_changed = false;
                let result = sync
                    .upload_or_verify(action, bar.clone(), &mut hash_missing, &mut file_changed)
                    .await;
                if let Some(bar) = bar {
                    bar.finish();
                }
                report.hash_missing += usize::from(hash_missing);
                if file_changed {
                    report.record_file_action(action, Some(cloud));
                }
                match result {
                    Ok(id) => {
                        report.skipped += usize::from(!file_changed);
                        retained_files.insert(rel_path.as_str(), id);
                    }
                    Err(error) => report.log_failure(rel_path, error),
                }
            }
            SyncAction::CreateDir {
                rel_path,
                target: SyncTarget::Cloud,
                ..
            } if !options.dry_run => {
                if let Err(error) = sync
                    .remote
                    .ensure_directory(&cloud_child_path(cloud_root, rel_path))
                    .await
                {
                    report.log_failure(rel_path, error);
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
            for path in deletes {
                report.record_deleted(path);
            }
        } else if let Err((path, error)) = sync
            .delete_verified_extras(&retained_files, &deletes, &mut report)
            .await
        {
            report.log_failure(path, error);
        }
    }
    report.seconds = started.elapsed().as_secs_f64();
    report
}

struct VerifiedSync<'a> {
    config: &'a Config,
    cloud: &'a CloudSyncScan,
    cloud_root: &'a str,
    remote: SyncCloud<'a>,
}

impl VerifiedSync<'_> {
    async fn confirm_root(&self) -> Result<()> {
        self.remote.ensure_directory(self.cloud_root).await?;
        Ok(())
    }

    async fn upload_or_verify(
        &self,
        action: &SyncAction,
        pb: Option<crate::services::progress::Progress>,
        hash_missing: &mut bool,
        file_changed: &mut bool,
    ) -> Result<String> {
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
        let remote_parent = cloud_child_path(self.cloud_root, parent);
        let parent_id = self.remote.ensure_directory(&remote_parent).await?;
        if *change == ChangeKind::Verify {
            let expected_id = self.cloud.expected_id(rel_path)?.context("原文件缺少 ID")?;
            self.remote
                .confirm_directory(&remote_parent, &parent_id)
                .await?;
            let file = self
                .remote
                .current_file(&parent_id, name, expected_id)
                .await?;
            if file.size.is_none_or(|remote_size| remote_size == size) {
                let path = local_abs.clone();
                let local_hash = tokio::task::spawn_blocking(move || {
                    crate::utils::crypto::calc_file_sha256(&path)
                })
                .await??;
                let remote_hash = remote_sha256(
                    self.config,
                    &self.cloud.host,
                    &file,
                    file.size,
                    hash_missing,
                )
                .await?;
                if local_hash == remote_hash {
                    self.remote
                        .confirm_directory(&remote_parent, &parent_id)
                        .await?;
                    let latest = self
                        .remote
                        .current_file(&parent_id, name, expected_id)
                        .await?;
                    ensure!(file == latest, "云端文件在校验期间发生变化");
                    return Ok(expected_id.to_owned());
                }
            }
        }

        upload_sync_to_parent(
            self.config,
            local_abs,
            SyncUploadPlan {
                host: &self.cloud.host,
                parent_id: &parent_id,
                remote_parent: &remote_parent,
                file_name: name,
                replace_directory_id: replace_directory_id.as_deref(),
                scan: Some(self.cloud),
                rel_path,
                hash_missing: Some(hash_missing),
                file_changed,
            },
            pb,
        )
        .await
    }

    async fn delete_verified_extras<'a>(
        &self,
        retained: &BTreeMap<&'a str, String>,
        deletes: &[&'a str],
        report: &mut SyncReport,
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
            let remote = cloud_child_path(self.cloud_root, parent);
            let expected_id = self
                .remote
                .ensure_directory(&remote)
                .await
                .map_err(|error| (path, error))?;
            self.remote
                .confirm_directory(&remote, &expected_id)
                .await
                .map_err(|error| (path, error))?;
            let listing = self.remote.listing().await.map_err(|error| (path, error))?;
            let current = listing
                .list_files(&expected_id)
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

        for (path, _) in delete_ids {
            self.remote
                .delete_extra(path)
                .await
                .map_err(|error| (path, error))?;
            report.record_deleted(path);
        }
        Ok(())
    }
}

fn parent_and_name(path: &str) -> (&str, &str) {
    path.rsplit_once('/').unwrap_or(("", path))
}
