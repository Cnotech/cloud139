use cloud139::{client::ClientError, config::Config};
use std::process::Command;

const EXPECTED: &str = "未实现：当前仅支持个人云";

#[tokio::test]
async fn non_personal_api_requests_are_rejected_before_transport() {
    use cloud139::{client::api, services};
    for storage in ["family", "group", "unknown", "personal", ""] {
        let profile = Config {
            storage_type: storage.into(),
            ..Config::default()
        };
        assert!(matches!(
            api::HttpClientWrapper::new()
                .personal_api_request::<serde_json::Value>(
                    &profile,
                    "invalid",
                    &serde_json::json!({}),
                )
                .await,
            Err(ClientError::UnsupportedStorage)
        ));
        assert!(matches!(
            services::mkdir(&profile, "/", services::mkdir_service::MkdirMode::Single).await,
            Err(ClientError::UnsupportedStorage)
        ));
        assert!(matches!(
            api::get_personal_download_link(&profile, "id").await,
            Err(ClientError::UnsupportedStorage)
        ));
    }
}

#[test]
fn config_requires_an_explicit_storage_type() {
    let profile = r#"account = "test"
authorization = "test"
"#;

    assert!(toml::from_str::<Config>(profile).is_err());
}

#[test]
fn config_errors_are_reported_consistently_across_commands() {
    let dir = tempfile::tempdir().unwrap();
    let profile = dir.path().join("profile.toml");
    for (contents, expected) in [
        (None, "配置文件未找到"),
        (Some("invalid TOML ["), "TOML 解析错误"),
    ] {
        if let Some(contents) = contents {
            std::fs::write(&profile, contents).unwrap();
        }
        for args in [&["ls"][..], &["sync", "local", "cloud:/"]] {
            let output = Command::new(env!("CARGO_BIN_EXE_cloud139"))
                .current_dir(dir.path())
                .arg("--config")
                .arg(&profile)
                .args(args)
                .output()
                .unwrap();
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert_eq!(output.status.code(), Some(1), "{args:?}: {stderr}");
            assert!(
                stderr.contains(&format!("配置错误: {expected}")),
                "{stderr}"
            );
        }
    }
}

#[test]
fn all_commands_reject_unsupported_profiles_before_renewal_or_io() {
    let dir = tempfile::tempdir().unwrap();
    let profile = dir.path().join("profile.toml");
    for storage in ["family", "group", "unknown", "personal", ""] {
        std::fs::write(&profile, format!(
            "account = \"test\"\nauthorization = \"test\"\nrefresh_token = \"test\"\nstorage_type = \"{storage}\"\n"
        )).unwrap();
        for args in [
            &["ls"][..],
            &["mkdir", "/folder"],
            &["upload", "missing"],
            &["download", "/file"],
            &["rm", "/file"],
            &["cp", "/source", "/target"],
            &["mv", "/source", "/target"],
            &["rename", "/source", "other"],
            &["sync", "missing", "cloud:/"],
            &["sync", "cloud:/", "missing", "--verify"],
        ] {
            let output = Command::new(env!("CARGO_BIN_EXE_cloud139"))
                .current_dir(dir.path())
                .arg("--config")
                .arg(&profile)
                .args(args)
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(1), "{args:?}");
            assert!(
                String::from_utf8_lossy(&output.stderr).contains(EXPECTED),
                "{args:?}"
            );
        }
        assert!(
            std::fs::read_to_string(&profile)
                .unwrap()
                .contains("refresh_token = \"test\"")
        );
    }
}

#[test]
fn invalid_command_arguments_are_rejected_before_token_refresh() {
    let dir = tempfile::tempdir().unwrap();
    let profile = dir.path().join("profile.toml");
    Config {
        token_expire_time: Some(0),
        ..Config::default()
    }
    .save_to(&profile)
    .unwrap();

    for (args, exit_code, expected) in [
        (
            &["sync", "cloud:/", "local", "--verify"][..],
            2,
            "--verify 要求目标路径以 cloud: 开头",
        ),
        (
            &["sync", "local", "cloud:/", "--verify", "-j", "2"],
            2,
            "--verify 只允许 -j 1",
        ),
        (&["rm", "/file"], 1, "请使用 --yes 参数确认删除"),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_cloud139"))
            .current_dir(dir.path())
            .arg("--config")
            .arg(&profile)
            .args(args)
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(exit_code), "{stdout}\n{stderr}");
        assert!(stderr.contains(expected), "{stderr}");
        // 配置已过期且缺少 refresh token，误触发续期会先打印告警。
        assert!(!stdout.contains("自动续期"), "{stdout}");
        assert!(!stderr.contains("自动续期"), "{stderr}");
    }
}
