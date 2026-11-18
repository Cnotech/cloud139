use cloud139::{client::ClientError, config::Config};
use std::process::Command;

const EXPECTED: &str = "未实现：当前仅支持个人云";

#[tokio::test]
async fn non_personal_api_requests_are_rejected_before_transport() {
    use cloud139::{client::api, domain::StorageType, services};
    let config = Config::default();
    for storage in [StorageType::Family, StorageType::Group] {
        assert!(matches!(
            api::HttpClientWrapper::new()
                .personal_api_request::<serde_json::Value>(
                    &config,
                    "invalid",
                    &serde_json::json!({}),
                    storage,
                )
                .await,
            Err(ClientError::UnsupportedStorage)
        ));
        let profile = Config {
            storage_type: storage.as_str().into(),
            ..config.clone()
        };
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
fn all_commands_reject_unsupported_profiles_before_renewal_or_io() {
    let dir = tempfile::tempdir().unwrap();
    let profile = dir.path().join("profile.toml");
    for storage in ["family", "group", "unknown"] {
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
            &["login", "--token", "invalid", "--storage-type", storage],
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
