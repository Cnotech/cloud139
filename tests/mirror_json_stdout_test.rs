//! `mirror --json` 的机器契约：**stdout 只有那一个 JSON 对象**，日志（含重试告警）走 stderr。
//!
//! 回归背景：曾经 `warn` 走 stdout，一次重试就把 JSON 顶出第一行，`jq` 直接失败、
//! 备份轮次被误判成 failure（mirror 实际是成功的）。

use std::io::Read;
use std::net::TcpListener;
use std::process::Command;
use std::thread;

/// 一个每次都直接断开连接的假服务端：让 CLI 走完重试，必然产生 warn。
///
/// 用断连（瞬时错误）而不是 500 响应：mirror 与上游共用 utils/retry.rs，
/// 只重试连接/超时/传输类失败；5xx 响应体解析失败属于确定性错误，不重试。
fn spawn_dropping_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut buf = [0u8; 8192];
            let _ = stream.read(&mut buf);
            drop(stream);
        }
    });
    format!("http://{addr}")
}

/// 造一份能直接用的配置：指向假服务端，且 token 远未到期（避免续期去连真网络）。
fn write_config(dir: &std::path::Path, host: &str) -> std::path::PathBuf {
    let path = dir.join("cloud139rc.toml");
    let content = format!(
        "authorization = \"test-authorization\"\naccount = \"13800138000\"\nstorage_type = \"personal_new\"\npersonal_cloud_host = \"{host}\"\ntoken_expire_time = 9999999999999\n"
    );
    std::fs::write(&path, content).expect("write config");
    path
}

fn run_mirror(
    args: &[&str],
    config: &std::path::Path,
    local: &std::path::Path,
) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_cloud139"));
    command
        .arg("-c")
        .arg(config)
        .arg("mirror")
        .arg(local)
        .arg("cloud:/")
        .args(args);
    command.output().expect("run cloud139")
}

/// 触发重试时，stdout 必须仍然是「一个完整可解析的 JSON」，告警只能在 stderr。
#[test]
fn json_stdout_stays_parseable_when_retrying() {
    let host = spawn_dropping_server();
    let tmp = tempfile::tempdir().expect("tmp");
    let config = write_config(tmp.path(), &host);
    let local = tmp.path().join("repo");
    std::fs::create_dir_all(&local).expect("repo dir");
    std::fs::write(local.join("config"), b"abc").expect("config file");

    let output = run_mirror(&["--json", "--retry-backoff", "0"], &config, &local);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // 1) stdout 只有一行、且整体能被 JSON 解析（这是调用方的契约）
    let lines: Vec<&str> = stdout.lines().filter(|l| !l.trim().is_empty()).collect();
    assert_eq!(
        lines.len(),
        1,
        "stdout 只应有那一个 JSON，实际 {} 行:\n{stdout}",
        lines.len()
    );
    let value: serde_json::Value = serde_json::from_str(lines[0])
        .unwrap_or_else(|err| panic!("stdout 不是 JSON: {err}\n{stdout}"));
    // 2) 失败的语义仍然如实表达：failures 至少一条、退出码 1
    assert_eq!(value["failures"].as_array().map(Vec::len), Some(1));
    // 七字段齐全（v49 契约）
    for field in [
        "objects",
        "uploaded",
        "skipped",
        "deleted",
        "hashMissing",
        "seconds",
        "failures",
    ] {
        assert!(value.get(field).is_some(), "missing field {field}");
    }
    assert_eq!(output.status.code(), Some(1));
    // 3) 重试告警必须落在 stderr
    assert!(
        stderr.contains("重试"),
        "重试告警应在 stderr，实际:\n{stderr}"
    );
}

/// 不开 --json 时行为不变：日志仍在 stdout（这次改动只影响机器可读模式）。
#[test]
fn non_json_mode_keeps_logs_on_stdout() {
    let host = spawn_dropping_server();
    let tmp = tempfile::tempdir().expect("tmp");
    let config = write_config(tmp.path(), &host);
    let local = tmp.path().join("repo");
    std::fs::create_dir_all(&local).expect("repo dir");
    std::fs::write(local.join("config"), b"abc").expect("config file");

    let output = run_mirror(&["--retry-backoff", "0"], &config, &local);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("重试"),
        "非 JSON 模式的日志仍在 stdout，实际:\n{stdout}"
    );
}
