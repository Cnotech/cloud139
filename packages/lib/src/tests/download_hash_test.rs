use super::personal_content_sha256;
use crate::config::{Config, personal::PersonalConfig};
use crate::error::ClientError;
use httpmock::prelude::*;
use serde_json::json;

#[tokio::test]
async fn content_hash_requires_the_listed_size() {
    let server = MockServer::start();
    let link = server.mock(|when, then| {
        when.method(POST)
            .path("/file/getDownloadUrl")
            .json_body(json!({"fileId": "file"}));
        then.json_body(json!({"success": true, "data": {"url": server.url("/blob")}}));
    });
    let blob = server.mock(|when, then| {
        when.method(GET).path("/blob");
        then.body("abc");
    });
    let config = Config::default();
    let config = PersonalConfig::new(&config).unwrap();
    let host = server.base_url();
    assert_eq!(
        personal_content_sha256(config, &host, "file", Some(3))
            .await
            .unwrap(),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    for (expected_size, message) in [(2, "远端文件大于预期大小"), (4, "远端文件大小与列表不一致")]
    {
        let error = personal_content_sha256(config, &host, "file", Some(expected_size))
            .await
            .unwrap_err();
        assert!(matches!(error, ClientError::Api(detail) if detail.contains(message)));
    }
    link.assert_calls(3);
    blob.assert_calls(3);
}
