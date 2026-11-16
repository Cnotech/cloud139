use crate::commands::CommandExit;
use crate::services::mirror_service::{self, MirrorOptions, MirrorReport};
use crate::services::sync_service::parse_sync_endpoint;
use crate::{debug, info};
use clap::Parser;
use std::io::IsTerminal;

#[derive(Parser, Debug, Clone)]
#[command(
    about = "把本地目录推到云端仓（只增不删，不删除远端已有对象）",
    long_about = "把本地目录推到云端仓：进程内完成遍历、差集、上传、校验、自愈。\n\n\
**只增不删**：远端有、本地没有的对象不会被删除（append-only），只在 JSON 的 remoteOnly 里报告数量。\n\n\
按文件名分流（不假定仓的类型）：\n\
- 文件名是 64 位十六进制 = 内容寻址对象，名字即内容 sha256；远端同名但大小/哈希不一致，说明远端那份坏了，删掉重传（自愈，次数由 --heal 限制）。\n\
- 其它文件名 = 普通文件；同名但内容不同、或远端拿不到 contentHash 时**记失败并退出 1**，不覆盖、不删除远端对象（路径配错时靠这条拦住，避免写坏别人的仓库）。\n\
- 普通文件缺哈希不会降级为「按相同处理」：证明不了相同就报错，这是有意的。\n\n\
远端仓根不存在时会逐级创建（不会用 mkdir -f 改名）。\n\n\
--json 时 stdout 只输出这一行 JSON，其余日志（含重试告警、上传进度行）全部走 stderr，调用方可以直接解析 stdout。\n\n\
退出码：0 全部成功；1 有对象失败（继续处理其它对象后统一返回）；2 参数或配置错误。"
)]
pub struct MirrorArgs {
    #[arg(help = "本地仓根目录，例如 /var/lib/beecodeai-backup/restic")]
    pub local: String,

    #[arg(help = "远端仓根，形如 cloud:/beecodeai-backup/prod/restic")]
    pub remote: String,

    #[arg(
        long = "no-verify",
        action = clap::ArgAction::SetFalse,
        help = "跳过 contentHash 校验（默认开启，只比 size 时用）"
    )]
    pub verify: bool,

    #[arg(long, default_value_t = 3, value_name = "N", help = "自愈上限次数")]
    pub heal: u32,

    #[arg(long, help = "只算差异，不发任何写请求")]
    pub dry_run: bool,

    #[arg(
        long,
        help = "输出 JSON 汇总（失败时也会输出）；此时 stdout 只有 JSON，日志走 stderr"
    )]
    pub json: bool,

    #[arg(long, default_value_t = 100, value_name = "N", help = "列目录每页条数")]
    pub page_size: i32,

    #[arg(
        long,
        default_value_t = 1,
        value_name = "N",
        help = "并发上传数；当前仅支持 1（>1 会因翻页漂移漏对象）"
    )]
    pub jobs: usize,

    #[arg(
        long,
        default_value_t = 5,
        value_name = "N",
        help = "列目录/上传各自的重试次数"
    )]
    pub retries: u32,

    #[arg(
        long,
        default_value_t = 3,
        value_name = "S",
        help = "重试退避基数（秒），按 3/6/12/24 指数增长"
    )]
    pub retry_backoff: u64,
}

pub async fn execute(args: MirrorArgs) -> anyhow::Result<()> {
    // 直接调用本命令时也保证 stdout 干净（main 已在分发前开过一次）。
    crate::utils::logger::set_json_mode(args.json);
    validate_args(&args)?;

    let config = crate::config::Config::load()
        .map_err(|err| CommandExit::new(2, format!("读取配置失败: {err}")))?;

    let options = MirrorOptions {
        verify: args.verify,
        heal: args.heal,
        dry_run: args.dry_run,
        page_size: args.page_size,
        retries: args.retries,
        retry_backoff_secs: args.retry_backoff,
    };

    debug!(
        "mirror: local={}, remote={}, verify={}, dry_run={}",
        args.local, args.remote, args.verify, args.dry_run
    );

    let report = match mirror_service::run(&config, &args.local, &args.remote, &options).await {
        Ok(report) => report,
        Err(err) => {
            // 契约：失败也要给调用方一份可解析的汇总（44659 用它写 latest.json）。
            let message = format!("{err:#}");
            if args.json {
                crate::print!("{}", fatal_report_json(&args.local, &message));
            }
            return Err(CommandExit::new(1, message).into());
        }
    };

    if args.json {
        crate::print!("{}", serde_json::to_string(&report)?);
    } else {
        info!(
            "对象 {} 个：上传 {}、跳过 {}、校验 {}、缺哈希 {}、自愈 {}、远端独有 {}、上传 {} 字节、耗时 {:.1}s",
            report.objects,
            report.uploaded,
            report.skipped,
            report.verified,
            report.hash_missing,
            report.healed,
            report.remote_only,
            report.bytes_uploaded,
            report.seconds
        );
        for failure in &report.failures {
            crate::error!(
                "失败 {}: {}（重试 {}）",
                failure.path,
                failure.error,
                failure.retries
            );
        }
        if !std::io::stdout().is_terminal() {
            // 非交互场景保持安静，退出码已经表达结果
        }
    }

    if !report.ok() {
        return Err(CommandExit::new(
            exit_code_for_report(&report),
            format!("{} 个对象处理失败", report.failures.len()),
        )
        .into());
    }
    Ok(())
}

/// 参数校验：全部返回 exit 2（参数错误），与「对象传输失败」的 1 区分开。
pub fn validate_args(args: &MirrorArgs) -> Result<(), CommandExit> {
    if args.jobs != 1 {
        return Err(CommandExit::new(
            2,
            "暂不支持 --jobs > 1：并发往同一父目录写会因翻页漂移漏对象，请保持串行（默认 1）",
        ));
    }
    if args.page_size <= 0 || args.page_size > 1000 {
        return Err(CommandExit::new(2, "--page-size 必须在 1..=1000 之间"));
    }
    if args.retry_backoff > 600 {
        return Err(CommandExit::new(2, "--retry-backoff 不能超过 600 秒"));
    }

    match parse_sync_endpoint(&args.remote) {
        crate::domain::SyncEndpoint::Cloud(_) => {}
        crate::domain::SyncEndpoint::Local(_) => {
            return Err(CommandExit::new(
                2,
                "远端必须是 cloud:/path 形式，例如 cloud:/beecodeai-backup/prod/restic",
            ));
        }
    }

    // 本地仓根不存在属于参数错误（exit 2），不能混进「对象失败」的 1。
    if !std::path::Path::new(&args.local).is_dir() {
        return Err(CommandExit::new(
            2,
            format!("本地仓根不存在或不是目录: {}", args.local),
        ));
    }
    Ok(())
}

/// 致命错误时的汇总：字段与正常输出一致，`failures` 放一条 fatal。
/// 调用方（备份脚本）靠它写 `latest.json`，所以失败也必须给。
pub fn fatal_report_json(local: &str, message: &str) -> serde_json::Value {
    serde_json::json!({
        "objects": 0,
        "uploaded": 0,
        "skipped": 0,
        "verified": 0,
        "hashMissing": 0,
        "healed": 0,
        "remoteOnly": 0,
        "bytesUploaded": 0,
        "seconds": 0.0,
        "failures": [{
            "path": local,
            "error": message,
            "retries": 0
        }]
    })
}

/// 对象传输失败 -> 1；全部成功 -> 0（参数/配置错误在更早的分支返回 2）。
pub fn exit_code_for_report(report: &MirrorReport) -> i32 {
    if report.ok() { 0 } else { 1 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::mirror_service::MirrorFailure;

    fn default_args() -> MirrorArgs {
        MirrorArgs {
            local: "/tmp".to_string(),
            remote: "cloud:/repo".to_string(),
            verify: true,
            heal: 3,
            dry_run: false,
            json: true,
            page_size: 100,
            jobs: 1,
            retries: 5,
            retry_backoff: 3,
        }
    }

    /// 致命错误（run() 返回 Err）时，调用方仍要拿到可解析的汇总：
    /// 字段齐全、failures 至少一条、带 path/error/retries。
    #[test]
    fn fatal_error_still_produces_parseable_json() {
        let value = fatal_report_json("/tmp/mt", "创建远端目录失败: boom");

        for field in [
            "objects",
            "uploaded",
            "skipped",
            "verified",
            "hashMissing",
            "healed",
            "remoteOnly",
            "bytesUploaded",
            "seconds",
            "failures",
        ] {
            assert!(value.get(field).is_some(), "missing field {field}");
        }

        let failures = value["failures"].as_array().expect("failures is array");
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0]["path"], "/tmp/mt");
        assert_eq!(failures[0]["error"], "创建远端目录失败: boom");
        assert_eq!(failures[0]["retries"], 0);
        // 必须能被反序列化，调用方才敢直接写 latest.json
        assert!(serde_json::to_string(&value).is_ok());
    }

    /// 退出码映射：参数错误 -> 2，对象失败 -> 1，全成功 -> 0。
    #[test]
    fn exit_codes_map_to_contract() {
        // 参数错误一律 2
        let mut args = default_args();
        args.jobs = 2;
        assert_eq!(validate_args(&args).unwrap_err().code(), 2);

        let mut args = default_args();
        args.page_size = 0;
        assert_eq!(validate_args(&args).unwrap_err().code(), 2);

        let mut args = default_args();
        args.retry_backoff = 601;
        assert_eq!(validate_args(&args).unwrap_err().code(), 2);

        let mut args = default_args();
        args.remote = "/local/path".to_string();
        assert_eq!(validate_args(&args).unwrap_err().code(), 2);

        let mut args = default_args();
        args.local = "/definitely/not/here-44768".to_string();
        assert_eq!(validate_args(&args).unwrap_err().code(), 2);

        // 合法参数通过校验
        assert!(validate_args(&default_args()).is_ok());

        // 对象传输失败 -> 1；无失败 -> 0
        let mut report = MirrorReport::default();
        assert_eq!(exit_code_for_report(&report), 0);
        report.failures.push(MirrorFailure {
            path: "data/aa/bb".to_string(),
            error: "boom".to_string(),
            retries: 5,
        });
        assert_eq!(exit_code_for_report(&report), 1);
    }
}
