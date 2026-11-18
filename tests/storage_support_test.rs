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
