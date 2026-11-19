use super::diff::SyncDiffOptions;
use super::executor::{SyncExecuteOptions, execute_scanned_sync};
use super::paths::{
    compile_exclude_patterns, normalize_sync_endpoint, parse_sync_endpoint, sync_direction,
};
use super::report::SyncReport;
use super::scan::{CloudSyncScan, SyncScanOptions, scan_personal_cloud};
use super::verify::execute_verified_sync;
use crate::config::{Config, PersonalConfig};
use crate::models::sync_item::{
    SyncAction, SyncDirection, SyncEndpoint, SyncEntryKind, SyncSummary,
};
use crate::progress::ProgressFactory;
use crate::services::local_scan::{LocalSyncScan, scan_local_target, scan_upload_source};
use anyhow::{Result, ensure};
use glob::Pattern;
use std::time::Instant;

/// 已校验的本地到云端或云端到本地同步参数。
pub struct SyncRequest {
    source: SyncEndpoint,
    destination: SyncEndpoint,
    direction: SyncDirection,
    recursive: bool,
    exclude: Vec<Pattern>,
    execution: SyncExecuteOptions,
    delete: bool,
    verify: bool,
}

/// 同步扫描结果和据此生成的操作计划。
pub struct SyncPlan {
    config: PersonalConfig<Config>,
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

/// 同步生成的摘要或文件变更明细。
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
        } else {
            execution.validate()?;
        }
        Ok(Self {
            source: normalize_sync_endpoint(source)?,
            destination: normalize_sync_endpoint(destination)?,
            direction,
            recursive: scan.recursive,
            exclude: compile_exclude_patterns(&scan.exclude)?,
            execution,
            delete,
            verify,
        })
    }

    pub async fn plan(self, config: Config, track_paths: bool) -> Result<SyncPlan> {
        let mut config = PersonalConfig::new(config)?;
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
                let (local_root, source) =
                    scan_upload_source(local_root, self.recursive, &self.exclude, self.delete)?;
                let objects = source
                    .iter()
                    .filter(|entry| entry.kind == SyncEntryKind::File)
                    .count();
                ensure!(
                    !self.verify || !self.delete || objects > 0,
                    "--verify --delete 要求同步范围内至少有一个源文件"
                );
                let cloud = scan_personal_cloud(
                    config.as_ref(),
                    &cloud_root,
                    self.recursive,
                    &self.exclude,
                    self.direction,
                )
                .await?;
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
                let cloud = scan_personal_cloud(
                    config.as_ref(),
                    &cloud_root,
                    self.recursive,
                    &self.exclude,
                    self.direction,
                )
                .await?;
                let local = scan_local_target(&local_root, self.recursive, &self.exclude)?;
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
        config.set_cloud_host(cloud.host.clone());
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
        let config = self.config.as_ref();
        match self.mode {
            SyncMode::Verify {
                objects,
                track_paths,
            } => {
                let report = execute_verified_sync(
                    config,
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
                    config,
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
