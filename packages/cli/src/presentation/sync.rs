use crate::{success, warn};
use cloud139::models::sync_item::{SyncAction, SyncSummary};
use cloud139::services::sync::{SyncOutcome, format_action_line};
use cloud139::services::sync_report::SyncReport;
use serde::Serialize;

pub(crate) fn print_plan(actions: &[SyncAction]) {
    for action in actions
        .iter()
        .filter(|action| !matches!(action, SyncAction::Skip { .. }))
    {
        crate::step!("{}", format_action_line(action, true));
    }
}

pub(crate) fn print_outcome(
    outcome: SyncOutcome,
    source: &str,
    destination: &str,
    dry_run: bool,
    json: bool,
) -> anyhow::Result<()> {
    match outcome {
        SyncOutcome::Summary(summary) => print_summary(&summary, dry_run),
        SyncOutcome::Report(mut report) if json => {
            print_json_report(source, destination, &mut report)?;
        }
        SyncOutcome::Report(report) => {
            let label = if dry_run {
                "校验计划"
            } else {
                "校验汇总"
            };
            crate::info!(
                "{label}: {} 个源文件，{} 个上传，{} 个跳过，{} 个删除，{} 个缺少可比哈希，{} 个失败",
                report.objects,
                report.uploaded,
                report.skipped,
                report.deleted,
                report.hash_missing,
                report.failures.len(),
            );
        }
    }
    Ok(())
}

fn print_json_report(
    source: &str,
    destination: &str,
    report: &mut SyncReport,
) -> anyhow::Result<()> {
    #[derive(Serialize)]
    struct Output<'a> {
        source: &'a str,
        destination: &'a str,
        #[serde(flatten)]
        report: &'a SyncReport,
    }

    report.sort_paths();
    crate::print!(
        "{}",
        serde_json::to_string(&Output {
            source,
            destination,
            report,
        })?
    );
    Ok(())
}

/// 打印同步结果摘要。
fn print_summary(summary: &SyncSummary, dry_run: bool) {
    let mut parts = Vec::new();
    if summary.transferred > 0 {
        parts.push(format!("{} 个文件传输", summary.transferred));
    }
    if summary.created_dirs > 0 {
        parts.push(format!("{} 个目录创建", summary.created_dirs));
    }
    if summary.deleted > 0 {
        parts.push(format!("{} 个删除", summary.deleted));
    }
    if summary.skipped > 0 {
        parts.push(format!("{} 个跳过", summary.skipped));
    }
    if summary.failed > 0 {
        parts.push(format!("{} 个失败", summary.failed));
    }
    if dry_run {
        crate::info!(
            "同步计划: {}",
            if parts.is_empty() {
                "无变化".to_owned()
            } else {
                parts.join(", ")
            }
        );
    } else if parts.is_empty() {
        success!("同步完成: 无变化");
    } else if summary.failed > 0 {
        warn!("同步未完成: {}", parts.join(", "));
    } else {
        success!("同步完成: {}", parts.join(", "));
    }
}
