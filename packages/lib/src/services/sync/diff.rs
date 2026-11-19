use super::paths::{is_recovery_path, path_and_ancestors, rel_to_local};
use super::scan::CloudSyncScan;
use crate::debug;
use crate::models::sync_item::{
    ChangeKind, FileEntry, SyncAction, SyncDirection, SyncEntryKind, SyncTarget,
};
use crate::services::local_scan::LocalSyncScan;
use crate::utils::remote_path;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct SyncDiffOptions {
    pub direction: SyncDirection,
    pub delete: bool,
    pub local_root: PathBuf,
    pub cloud_root: String,
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
            || cloud_target.is_some_and(|scan| scan.is_protected(path))
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
                    let directory_id = cloud_target.and_then(|scan| scan.directory_id(path));
                    actions.push(transfer_action(
                        src,
                        &options,
                        ChangeKind::New,
                        directory_id.map(str::to_owned),
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

    // 上传文件可能会替换同名云端目录。替换成功前保留旧目录的所有子条目，
    // 将旧目录移入回收站时，其子条目也会一同移入。
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
    // 已有目录直接复用，目录内容的变更按子条目处理。
    if source.kind == SyncEntryKind::Directory && target.kind == SyncEntryKind::Directory {
        return None;
    }

    if source.size != target.size {
        return Some(ChangeKind::SizeOrTime);
    }

    // 本地到云端同步时，本地修改时间和云端记录的上传时间含义不同，
    // 因此只比较文件大小。
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

impl CloudSyncScan {
    pub(super) fn compute_upload_diff(
        &self,
        source: &[FileEntry],
        options: SyncDiffOptions,
        verify: bool,
    ) -> Vec<SyncAction> {
        compute_diff_inner(source, &self.items, options, Some(self), None, verify)
    }

    pub(super) fn compute_download_diff(
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
}
