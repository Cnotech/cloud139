use crate::cli::LoginArgs;
use cloud139::config::Config;
use std::{future::Future, path::Path};

pub(super) async fn execute(args: LoginArgs, path: &Path) -> anyhow::Result<()> {
    save_after_verification(path, cloud139::services::login::login(&args.token)).await
}

async fn save_after_verification(
    path: &Path,
    verification: impl Future<Output = anyhow::Result<Config>>,
) -> anyhow::Result<()> {
    let config = verification.await?;
    crate::config::store::save_config(&config, path)?;
    crate::success!("Token 验证成功!");
    crate::success!("配置文件已保存到: {}", path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::save_after_verification;
    use cloud139::error::ClientError;
    use std::{fs, future};

    #[tokio::test]
    async fn rejected_login_does_not_create_or_overwrite_config() {
        let directory = tempfile::tempdir().unwrap();
        let missing = directory.path().join("missing.toml");
        let existing = directory.path().join("existing.toml");
        let original = b"account = \"test-account\"\nauthorization = \"test-auth\"\nstorage_type = \"personal_new\"\n";
        fs::write(&existing, original).unwrap();

        let rejection = "模拟服务端拒绝凭据";
        for path in [&missing, &existing] {
            let verification = future::ready(Err(ClientError::Api(rejection.into()).into()));
            let error = save_after_verification(path, verification)
                .await
                .unwrap_err();
            assert!(matches!(error.downcast_ref::<ClientError>(),
                Some(ClientError::Api(message)) if message == rejection));
        }

        assert!(!missing.exists());
        assert_eq!(fs::read(existing).unwrap(), original);
    }
}
