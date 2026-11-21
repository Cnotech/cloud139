use super::executor::{SyncExecuteOptions, make_action_progress};
use super::publish::{SyncUploadPlan, upload_sync_to_parent};
use super::remote::{SyncCloud, remote_sha256};
use super::report::SyncReport;
use super::scan::{CloudSyncFile, CloudSyncScan};
use crate::config::{Config, personal::PersonalConfig};
use crate::models::sync_item::{ChangeKind, SyncAction, SyncTarget};
use crate::progress::{Progress, ProgressFactory};
use crate::services::upload::checked_upload_size;
use crate::utils::crypto::{calc_file_sha256, is_sha256_hex};
use crate::utils::remote_path::cloud_child_path;
use anyhow::{Context, Result, anyhow, ensure};
use std::collections::{BTreeMap, HashSet};
use std::time::Instant;

pub(super) async fn execute_verified_sync(
    config: PersonalConfig<&Config>,
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
    config: PersonalConfig<&'a Config>,
    cloud: &'a CloudSyncScan,
    cloud_root: &'a str,
    remote: SyncCloud<'a>,
}

impl VerifiedSync<'_> {
    async fn upload_or_verify(
        &self,
        action: &SyncAction,
        pb: Option<Progress>,
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
        let size = checked_upload_size(local_abs)? as u64;
        let (parent, name) = parent_and_name(rel_path);
        let scanned_file = if *change == ChangeKind::Verify {
            Some(self.cloud.file(rel_path).context("扫描缺少原文件")?)
        } else {
            None
        };
        let comparable =
            scanned_file.filter(|file| file.size.is_none_or(|remote_size| remote_size == size));
        let content_addressed = is_sha256_hex(name);
        if content_addressed || comparable.is_some() {
            let path = local_abs.clone();
            let local_hash = tokio::task::spawn_blocking(move || calc_file_sha256(&path)).await??;
            ensure!(
                !content_addressed || local_hash.eq_ignore_ascii_case(name),
                "内容寻址对象 {name} 的实际内容哈希 {local_hash} 与文件名不一致，已保留远端旧副本"
            );
            if let Some(file) = comparable {
                let expected_id = file.id.as_deref().context("扫描未返回有效文件 ID")?;
                let remote_hash =
                    remote_sha256(self.config, &self.cloud.host, file, file.size, hash_missing)
                        .await?;
                if local_hash == remote_hash {
                    return Ok(expected_id.to_owned());
                }
            }
        }

        let remote_parent = cloud_child_path(self.cloud_root, parent);
        let parent_id = match self.cloud.directory_id(parent) {
            Some(id) => {
                self.remote
                    .confirm_scanned_directory(&remote_parent, id)
                    .await?;
                id.to_owned()
            }
            None => self.remote.ensure_directory(&remote_parent).await?,
        };
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
        let retained_ids: HashSet<_> = retained.values().map(String::as_str).collect();
        let mut deleted_ids = HashSet::new();
        let mut delete_ids = Vec::with_capacity(deletes.len());
        let mut retained_by_parent = BTreeMap::<&str, Vec<(&str, &str)>>::new();
        for (&path, id) in retained {
            retained_by_parent
                .entry(parent_and_name(path).0)
                .or_default()
                .push((path, id));
        }
        let mut deletes_by_parent = BTreeMap::<&str, Vec<(&str, &str)>>::new();
        for &path in deletes {
            let id = self
                .cloud
                .file(path)
                .and_then(|file| file.id.as_deref())
                .ok_or_else(|| (path, anyhow!("计划删除的文件缺少扫描 ID")))?;
            if retained_ids.contains(id) || !deleted_ids.insert(id) {
                return Err((path, anyhow!("计划删除的文件 ID 重复或与保留文件相同")));
            }
            deletes_by_parent
                .entry(parent_and_name(path).0)
                .or_default()
                .push((path, id));
            delete_ids.push((path, id));
        }

        // 只复核本次真正要删除的目录：发首个回收请求前，把这些目录里的保留项
        // 和待删项一起核对；无关目录不再重新列举。
        for (parent, deletes_in_parent) in deletes_by_parent {
            let path = deletes_in_parent[0].0;
            let remote = cloud_child_path(self.cloud_root, parent);
            let expected_id = self
                .cloud
                .directory_id(parent)
                .ok_or_else(|| (path, anyhow!("待删除条目的父目录缺少扫描 ID")))?;
            self.remote
                .confirm_scanned_directory(&remote, expected_id)
                .await
                .map_err(|error| (path, error))?;
            let listing = self.remote.listing().await.map_err(|error| (path, error))?;
            let current = listing
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
            let mut files = retained_by_parent.get(parent).cloned().unwrap_or_default();
            files.extend(deletes_in_parent);
            for (path, expected_id) in files {
                let name = parent_and_name(path).1;
                let scanned_file = self
                    .cloud
                    .file(path)
                    .filter(|file| file.id.as_deref() == Some(expected_id));
                if !by_name.get(name).is_some_and(|item| {
                    !item.is_folder()
                        && item.file_id.as_deref() == Some(expected_id)
                        && scanned_file.is_none_or(|file| CloudSyncFile::from(*item) == *file)
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
