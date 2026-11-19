use super::sync_service::CloudSyncScan;
use crate::domain::{ChangeKind, FileEntry, SyncAction, SyncEntryKind};
use crate::error::RetryError;
use serde::Serialize;
use std::collections::HashSet;

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncReport {
    pub objects: usize,
    pub uploaded: usize,
    pub downloaded: usize,
    pub skipped: usize,
    pub deleted: usize,
    pub hash_missing: usize,
    pub seconds: f64,
    pub failures: Vec<SyncFailure>,
    pub dry_run: bool,
    pub changes: Vec<SyncFileChange>,
    pub plan: Vec<SyncFileChange>,
    #[serde(skip)]
    track_paths: bool,
}

#[derive(Serialize)]
pub struct SyncFailure {
    pub path: String,
    pub error: String,
    pub retries: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileOperation {
    Upload,
    Download,
    Delete,
    Verify,
}

#[derive(Serialize)]
pub struct SyncFileChange {
    pub path: String,
    pub action: FileOperation,
}

impl SyncReport {
    pub(super) fn new(dry_run: bool, track_paths: bool) -> Self {
        Self {
            dry_run,
            track_paths,
            ..Self::default()
        }
    }

    pub(super) fn count_source_files(&mut self, source: &[FileEntry], actions: &[SyncAction]) {
        let skipped: HashSet<_> = actions
            .iter()
            .filter_map(|action| match action {
                SyncAction::Skip { rel_path } => Some(rel_path.as_str()),
                _ => None,
            })
            .collect();
        for entry in source
            .iter()
            .filter(|entry| entry.kind == SyncEntryKind::File)
        {
            self.objects += 1;
            self.skipped += usize::from(skipped.contains(entry.rel_path.as_str()));
        }
    }

    pub(super) fn record_file_action(
        &mut self,
        action: &SyncAction,
        cloud: Option<&CloudSyncScan>,
    ) {
        match action {
            SyncAction::Upload {
                rel_path,
                change: ChangeKind::Verify,
                ..
            } if self.dry_run => self.record(FileOperation::Verify, rel_path),
            SyncAction::Upload {
                rel_path,
                replace_directory_id,
                ..
            } => {
                self.record(FileOperation::Upload, rel_path);
                if self.track_paths
                    && replace_directory_id.is_some()
                    && let Some(cloud) = cloud
                {
                    for path in cloud.descendant_file_paths(rel_path) {
                        self.record_deleted(path);
                    }
                }
            }
            SyncAction::Download { rel_path, .. } => self.record(FileOperation::Download, rel_path),
            SyncAction::Delete { rel_path, .. } => self.record_deleted(rel_path),
            _ => {}
        }
    }

    pub(super) fn record_deleted(&mut self, path: &str) {
        self.record(FileOperation::Delete, path);
    }

    fn record(&mut self, action: FileOperation, path: &str) {
        match action {
            FileOperation::Upload => self.uploaded += 1,
            FileOperation::Download => self.downloaded += 1,
            FileOperation::Delete => self.deleted += 1,
            FileOperation::Verify => {}
        }
        if self.track_paths {
            let entries = if self.dry_run {
                &mut self.plan
            } else {
                &mut self.changes
            };
            entries.push(SyncFileChange {
                path: path.into(),
                action,
            });
        }
    }

    pub(super) fn record_failure(&mut self, path: &str, error: &anyhow::Error, message: String) {
        let retries = error
            .downcast_ref::<RetryError>()
            .map_or(0, |error| error.attempts);
        self.failures.push(SyncFailure {
            path: path.into(),
            error: message,
            retries,
        });
    }

    pub(super) fn log_failure(&mut self, path: &str, error: anyhow::Error) {
        let message = format!("{error:#}");
        crate::error!("同步失败: {path}: {message}");
        self.record_failure(path, &error, message);
    }

    pub fn sort_paths(&mut self) {
        for entries in [&mut self.changes, &mut self.plan] {
            entries.sort_unstable_by(|left, right| {
                left.path
                    .cmp(&right.path)
                    .then(left.action.cmp(&right.action))
            });
        }
        self.failures
            .sort_by(|left, right| left.path.cmp(&right.path));
    }
}
