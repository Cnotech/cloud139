use crate::cli::SyncArgs;
use crate::error::CommandExit;
use crate::presentation::sync::{print_outcome, print_plan};
use cloud139::config::Config;
use cloud139::progress::ProgressFactory;
use cloud139::services::sync::executor::SyncExecuteOptions;
use cloud139::services::sync::plan::SyncRequest;
use cloud139::services::sync::scan::SyncScanOptions;
use std::io::IsTerminal;

pub(super) fn preflight(args: &SyncArgs) -> Result<SyncRequest, CommandExit> {
    let progress = show_progress(args);
    SyncRequest::new(
        &args.src,
        &args.dest,
        SyncScanOptions {
            recursive: args.recursive,
            exclude: args.exclude.clone(),
        },
        SyncExecuteOptions {
            dry_run: args.dry_run,
            jobs: args.jobs,
            print_actions: !progress && !args.dry_run,
        },
        args.delete,
        args.verify,
    )
    .map_err(|error| CommandExit::new(2, error.to_string()))
}

pub(super) async fn execute(
    args: SyncArgs,
    request: SyncRequest,
    config: Config,
) -> anyhow::Result<()> {
    crate::debug!("开始同步扫描 event=sync_phase_start phase=scan");
    let plan = request.plan(config, args.json).await;
    crate::debug!(
        "结束同步扫描 event=sync_phase_end phase=scan ok={}",
        plan.is_ok()
    );
    let plan = plan.map_err(|error| CommandExit::new(2, error.to_string()))?;
    if args.dry_run {
        print_plan(plan.actions());
    }
    let progress: Option<&ProgressFactory<'_>> = if show_progress(&args) {
        Some(&crate::presentation::progress::make_progress)
    } else {
        None
    };
    crate::debug!("开始同步执行 event=sync_phase_start phase=execute");
    let outcome = plan.execute_with_progress(progress).await;
    crate::debug!(
        "结束同步执行 event=sync_phase_end phase=execute ok={}",
        outcome.as_ref().is_ok_and(|result| result.failed() == 0)
    );
    let outcome = outcome?;
    let failed = outcome.failed();
    print_outcome(outcome, &args.src, &args.dest, args.dry_run, args.json)?;
    if !args.dry_run && failed > 0 {
        return Err(CommandExit::new(1, format!("同步未完成: {failed} 个失败")).into());
    }
    Ok(())
}

fn show_progress(args: &SyncArgs) -> bool {
    !args.json && std::io::stderr().is_terminal()
}
