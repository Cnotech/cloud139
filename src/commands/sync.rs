use crate::client::ClientError;
use crate::commands::CommandExit;
use crate::config::Config;
use crate::presentation::sync_renderer::{print_outcome, print_plan};
use crate::services::sync_executor::{MAX_JOBS, SyncExecuteOptions};
use crate::services::sync_service::{SyncRequest, SyncScanOptions};
use clap::Parser;
use std::io::IsTerminal;

fn parse_jobs(value: &str) -> Result<usize, String> {
    let jobs = value
        .parse::<usize>()
        .map_err(|_| "并发数量必须为正整数".to_string())?;
    if !(1..=MAX_JOBS).contains(&jobs) {
        return Err(format!("并发数量必须介于 1 和 {MAX_JOBS} 之间"));
    }
    Ok(jobs)
}

#[derive(Parser, Debug, Clone)]
pub struct SyncArgs {
    #[arg(help = "源路径，本地路径或 cloud:/remote/path")]
    pub src: String,

    #[arg(help = "目标路径，本地路径或 cloud:/remote/path")]
    pub dest: String,

    #[arg(short = 'r', long, help = "递归同步子目录，空目录也会同步")]
    pub recursive: bool,

    #[arg(short = 'n', long, help = "只预览同步计划")]
    pub dry_run: bool,

    #[arg(long, help = "删除目标中源没有的文件或空目录")]
    pub delete: bool,

    #[arg(long, value_name = "PAT", help = "排除匹配的路径，可多次指定")]
    pub exclude: Vec<String>,

    #[arg(short = 'j', long, default_value_t = 4, default_value_if("verify", "true", Some("1")),
        value_parser = parse_jobs, help = format!("同时传输的文件数，范围 1 到 {MAX_JOBS}。默认 4，--verify 固定为 1"))]
    pub jobs: usize,

    #[arg(long, help = "校验内容并修复不一致的文件")]
    pub verify: bool,

    #[arg(long, help = "以 JSON 报告本次同步的文件变更")]
    pub json: bool,
}

pub async fn execute(args: SyncArgs) -> anyhow::Result<()> {
    let request = preflight(&args)?;
    let config = Config::load_personal().map_err(config_error)?;
    execute_with_config(args, request, config).await
}

pub(super) fn preflight(args: &SyncArgs) -> Result<SyncRequest, CommandExit> {
    let progress = !args.json && std::io::stderr().is_terminal();
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
            progress,
            print_actions: !progress && !args.dry_run,
        },
        args.delete,
        args.verify,
    )
    .map_err(|error| CommandExit::new(2, error.to_string()))
}

pub(super) fn config_error(error: ClientError) -> anyhow::Error {
    match error {
        ClientError::UnsupportedStorage => error.into(),
        _ => CommandExit::new(2, error.to_string()).into(),
    }
}

pub(super) async fn execute_with_config(
    args: SyncArgs,
    request: SyncRequest,
    config: Config,
) -> anyhow::Result<()> {
    let plan = request
        .plan(config, args.json)
        .await
        .map_err(|error| CommandExit::new(2, error.to_string()))?;
    if args.dry_run {
        print_plan(plan.actions());
    }
    let outcome = plan.execute().await?;
    let failed = outcome.failed();
    print_outcome(outcome, &args.src, &args.dest, args.dry_run, args.json)?;
    if !args.dry_run && failed > 0 {
        return Err(CommandExit::new(1, format!("同步未完成: {failed} 个失败")).into());
    }
    Ok(())
}
