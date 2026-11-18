use cloud139::{
    client::ClientError,
    domain::{PERSONAL_UPLOAD_MAX_BYTES, StorageType},
    services::upload_service::validate_upload_size,
};

#[test]
fn personal_limit_is_inclusive_and_does_not_limit_other_backends() {
    for size in [0, PERSONAL_UPLOAD_MAX_BYTES - 1, PERSONAL_UPLOAD_MAX_BYTES] {
        validate_upload_size(StorageType::PersonalNew, size).unwrap();
    }
    for size in [PERSONAL_UPLOAD_MAX_BYTES + 1, u64::MAX] {
        assert!(matches!(
            validate_upload_size(StorageType::PersonalNew, size),
            Err(ClientError::UploadTooLarge { .. })
        ));
        validate_upload_size(StorageType::Family, size).unwrap();
        validate_upload_size(StorageType::Group, size).unwrap();
    }
}

#[cfg(unix)]
#[tokio::test]
async fn oversized_single_upload_is_rejected_before_any_api_request_or_hashing() {
    use cloud139::config::Config;
    use cloud139::services::{upload::personal, upload_service};
    use httpmock::prelude::*;
    let server = MockServer::start();
    let requests = server.mock(|when, then| {
        when.any_request();
        then.status(500);
    });
    let config = Config {
        personal_cloud_host: Some(server.base_url()),
        ..Config::default()
    };
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("large.bin");
    let file = std::fs::File::create(&path).unwrap();
    file.set_len(PERSONAL_UPLOAD_MAX_BYTES).unwrap();
    assert_eq!(
        upload_service::checked_upload_size(StorageType::PersonalNew, &path).unwrap(),
        PERSONAL_UPLOAD_MAX_BYTES as i64
    );
    file.set_len(PERSONAL_UPLOAD_MAX_BYTES + 1).unwrap();
    for stale_size in [1, (PERSONAL_UPLOAD_MAX_BYTES + 1) as i64] {
        let error =
            upload_service::upload(&config, &path, "/", "large.bin", stale_size, true, None)
                .await
                .unwrap_err();
        assert!(matches!(error, ClientError::UploadTooLarge { .. }));
        let error = personal::upload(&config, &path, "/", "large.bin", stale_size, true, None)
            .await
            .unwrap_err();
        assert!(matches!(error, ClientError::UploadTooLarge { .. }));
    }
    requests.assert_calls(0);
}
