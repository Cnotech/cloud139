use cloud139::config::Config;
use httpmock::prelude::*;
use serde_json::json;
use std::fs;
use std::path::Path;
use std::process::Command;

const AUTHORIZATION: &str = "fixture-private-authorization";
const REFRESH_TOKEN: &str = "fixture-private-refresh-token";
const SIGNATURE: &str = "fixture-private-signature";

fn command(root: &Path, host: String) -> Command {
    let profile = root.join("profile.toml");
    Config {
        account: "13800138000".into(),
        storage_type: "personal_new".into(),
        authorization: AUTHORIZATION.into(),
        refresh_token: Some(REFRESH_TOKEN.into()),
        personal_cloud_host: Some(host),
        token_expire_time: Some(i64::MAX),
        ..Config::default()
    }
    .save_to(&profile)
    .unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_cloud139"));
    command.current_dir(root).arg("--config").arg(profile);
    command
}

#[test]
fn sync_log_file_counts_pages_and_keeps_stdout_json() {
    let root = tempfile::tempdir().unwrap();
    let server = MockServer::start();
    let first = server.mock(|when, then| {
        when.method(POST)
            .path("/file/list")
            .json_body_includes(json!({"pageInfo":{"pageCursor":""}}).to_string());
        then.json_body(json!({"success":true,"data":{"items":[],"nextPageCursor":"second"}}));
    });
    let second = server.mock(|when, then| {
        when.method(POST)
            .path("/file/list")
            .json_body_includes(json!({"pageInfo":{"pageCursor":"second"}}).to_string());
        then.json_body(json!({"success":true,"data":{"items":[]}}));
    });
    fs::create_dir(root.path().join("source")).unwrap();
    let output = command(root.path(), server.base_url())
        .args([
            "sync",
            "source",
            "cloud:/",
            "--verify",
            "--json",
            "--log-file",
            "requests.log",
        ])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.lines().count(), 1);
    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(report["failures"], json!([]));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("event=sync_phase_start"));
    first.assert_calls(1);
    second.assert_calls(1);

    let log = fs::read_to_string(root.path().join("requests.log")).unwrap();
    let starts: Vec<_> = log
        .lines()
        .filter(|line| line.contains("event=http_request_start"))
        .collect();
    let ends: Vec<_> = log
        .lines()
        .filter(|line| line.contains("event=http_request_end"))
        .collect();
    assert_eq!(starts.len(), 2);
    assert_eq!(ends.len(), starts.len());
    for (start, end) in starts.into_iter().zip(ends) {
        assert!(start.contains("method=POST endpoint=/file/list"));
        assert!(end.contains(field(start, "request_id=")));
        assert!(end.contains("status=200"));
        assert!(end.contains("outcome=ok"));
        let elapsed = field(end, "elapsed_ms=").trim_start_matches("elapsed_ms=");
        assert!(elapsed.parse::<f64>().unwrap() >= 0.0);
    }
    let (_, execute) = log
        .split_once("event=sync_phase_start phase=execute")
        .unwrap();
    assert!(!execute.contains("event=http_request_start"));
    assert!(execute.contains("event=sync_phase_end phase=execute ok=true"));
    assert!(!log.contains('\x1b'));
    assert!(!log.contains(AUTHORIZATION));
    assert!(!log.contains(REFRESH_TOKEN));
}

fn field<'a>(line: &'a str, prefix: &str) -> &'a str {
    line.split_whitespace()
        .find(|key| key.starts_with(prefix))
        .unwrap()
}

#[test]
fn http_429_is_logged_and_flushed_on_error_exit() {
    let root = tempfile::tempdir().unwrap();
    let server = MockServer::start();
    let limited = server.mock(|when, then| {
        when.method(POST).path("/file/list");
        then.status(429).header("Retry-After", "1");
    });
    fs::create_dir(root.path().join("source")).unwrap();
    let output = command(root.path(), server.base_url())
        .args([
            "--log-file",
            "error.log",
            "sync",
            "source",
            "cloud:/",
            "--json",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    limited.assert_calls(1);
    let log = fs::read_to_string(root.path().join("error.log")).unwrap();
    assert_eq!(log.matches("event=http_request_start").count(), 1);
    assert_eq!(log.matches("event=http_request_end").count(), 1);
    assert!(log.contains("endpoint=/file/list status=429"));
    assert!(log.contains("outcome=http_error"));
    assert!(log.contains("exit_code=2"));
    assert!(log.contains("error 操作失败"));
}

#[test]
fn download_logs_the_cdn_request_without_the_signed_link() {
    let root = tempfile::tempdir().unwrap();
    let server = MockServer::start();
    let list = server.mock(|when, then| {
        when.method(POST).path("/file/list");
        then.json_body(json!({"success":true,"data":{"items":[
            {"fileId":"item","name":"movie.bin","type":"file","size":4}
        ]}}));
    });
    let link = server.mock(|when, then| {
        when.method(POST).path("/file/getDownloadUrl");
        then.json_body(json!({"success":true,"data":{
            "url":server.url(format!("/payload?sign={SIGNATURE}"))
        }}));
    });
    let download = server.mock(|when, then| {
        when.method(GET)
            .path("/payload")
            .query_param("sign", SIGNATURE);
        then.status(403);
    });
    let output = command(root.path(), server.base_url())
        .args(["--log-file", "download.log", "download", "/movie.bin"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    list.assert_calls(1);
    link.assert_calls(1);
    download.assert_calls(1);
    let log = fs::read_to_string(root.path().join("download.log")).unwrap();
    assert!(log.contains("method=GET endpoint=cdn_download status=403"));
    assert!(!log.contains(SIGNATURE));
    assert!(!String::from_utf8_lossy(&output.stderr).contains(SIGNATURE));
}

#[test]
fn log_file_omits_untrusted_api_error_details() {
    let root = tempfile::tempdir().unwrap();
    let server = MockServer::start();
    let rejected = server.mock(|when, then| {
        when.method(POST).path("/file/list");
        then.json_body(json!({"success":false,"message":format!(
            "Authorization: {AUTHORIZATION} Cookie: {REFRESH_TOKEN} https://cdn.invalid/?sign={SIGNATURE}"
        )}));
    });
    let output = command(root.path(), server.base_url())
        .args(["--log-file", "rejected.log", "ls"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    rejected.assert_calls(1);
    let log = fs::read_to_string(root.path().join("rejected.log")).unwrap();
    assert!(log.contains("endpoint=/file/list status=200"));
    assert!(log.contains("error 操作失败"));
    for secret in [AUTHORIZATION, REFRESH_TOKEN, SIGNATURE] {
        assert!(!log.contains(secret));
    }
}

#[test]
fn log_path_conflicts_are_rejected_before_requests() {
    let root = tempfile::tempdir().unwrap();
    let server = MockServer::start();
    let unexpected = server.mock(|when, then| {
        when.path("/file/list");
        then.status(500);
    });
    fs::create_dir(root.path().join("source")).unwrap();
    let existing = root.path().join("existing.log");
    fs::write(&existing, "已有内容").unwrap();
    for (args, expected) in [
        (
            &["--log-file", "existing.log", "ls"][..],
            "无法新建日志文件",
        ),
        (
            &["--log-file", "report.json", "ls", "-o", "./report.json"][..],
            "列表输出文件冲突",
        ),
        (
            &["--log-file", "source/trace.log", "upload", "./source"][..],
            "上传源冲突",
        ),
        (
            &[
                "--log-file",
                "source/trace.log",
                "sync",
                "source",
                "cloud:/",
            ][..],
            "同步源冲突",
        ),
        (
            &["--log-file", "movie.bin", "download", "/movie.bin"][..],
            "下载目标冲突",
        ),
    ] {
        let output = command(root.path(), server.base_url())
            .args(args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected),
            "{output:?}"
        );
    }
    let output = Command::new(env!("CARGO_BIN_EXE_cloud139"))
        .current_dir(root.path())
        .args([
            "--config",
            "future.toml",
            "--log-file",
            "./future.toml",
            "ls",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("配置文件冲突"));
    assert!(!root.path().join("future.toml").exists());
    assert!(!root.path().join("report.json").exists());
    assert!(!root.path().join("source/trace.log").exists());
    assert!(!root.path().join("movie.bin").exists());
    assert_eq!(fs::read_to_string(existing).unwrap(), "已有内容");
    unexpected.assert_calls(0);
}

#[test]
fn log_path_conflicts_respect_case_insensitive_file_systems() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("case-probe"), "").unwrap();
    if !root.path().join("CASE-PROBE").exists() {
        return;
    }
    fs::create_dir(root.path().join("source")).unwrap();
    for (args, label, candidate) in [
        (
            &["--log-file", "REPORT.JSON", "ls", "-o", "report.json"][..],
            "列表输出文件",
            "REPORT.JSON",
        ),
        (
            &["--log-file", "SOURCE/log", "upload", "source"][..],
            "上传源",
            "source/log",
        ),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_cloud139"))
            .current_dir(root.path())
            .args(args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(label),
            "{output:?}"
        );
        assert!(!root.path().join(candidate).exists());
    }
}
