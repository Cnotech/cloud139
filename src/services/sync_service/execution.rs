use super::{
    CloudSyncScan, LocalSyncScan, SyncDiffOptions, SyncScanOptions, parse_sync_endpoint,
    scan_cloud, scan_local_target, scan_upload_source, sync_direction,
};
use crate::config::Config;
use crate::domain::{SyncAction, SyncDirection, SyncEndpoint, SyncEntryKind, SyncSummary};
use crate::services::sync_executor::{SyncExecuteOptions, execute_scanned_sync};
use crate::services::sync_report::SyncReport;
use crate::services::sync_verify::execute_verified_sync;
use anyhow::{Result, ensure};
use std::time::Instant;

pub(crate) struct SyncRequest {
    source: SyncEndpoint,
    destination: SyncEndpoint,
    direction: SyncDirection,
    scan: SyncScanOptions,
    execution: SyncExecuteOptions,
    delete: bool,
    verify: bool,
}

impl SyncRequest {
    pub(crate) fn new(
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

    pub(crate) async fn plan(self, mut config: Config, track_paths: bool) -> Result<SyncPlan> {
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
                let (local_root, source) = scan_upload_source(local_root, &self.scan, self.delete)?;
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
                let local = scan_local_target(&local_root, &self.scan)?;
                local.validate_destinations(&cloud, &local_root)?;
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

pub(crate) struct SyncPlan {
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

impl SyncPlan {
    pub(crate) fn actions(&self) -> &[SyncAction] {
        &self.actions
    }

    pub(crate) async fn execute(self) -> Result<SyncOutcome> {
        match self.mode {
            SyncMode::Verify {
                objects,
                track_paths,
            } => {
                let report = execute_verified_sync(
                    &self.config,
                    &self.cloud,
                    &self.cloud.root,
                    &self.actions,
                    objects,
                    self.execution,
                    track_paths,
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

pub(crate) enum SyncOutcome {
    Summary(SyncSummary),
    Report(SyncReport),
}

impl SyncOutcome {
    pub(crate) fn failed(&self) -> usize {
        match self {
            Self::Summary(summary) => summary.failed,
            Self::Report(report) => report.failures.len(),
        }
    }
}
