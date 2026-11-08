//! 续期逻辑的测试：解析、重建 authorization、阈值判断。

use base64::Engine;
use httpmock::Method::POST;
use httpmock::prelude::*;

use cloud139::client::auth;
use cloud139::config::{Config, TOKEN_REFRESH_THRESHOLD_MS};

const REFRESH_PATH: &str = "/tellin/authTokenRefresh.do";

fn base_config() -> Config {
    Config {
        account: "13800138000".to_string(),
        authorization: "old-authorization".to_string(),
        refresh_token: Some("old-refresh-token".to_string()),
        storage_type: "personal_new".to_string(),
        ..Default::default()
    }
}

/// 真实接口的响应形状：新 token 在 <token>，有效期在 <expiretime>，<accessToken> 为空。
fn success_body(token: &str, expire_secs: i64) -> String {
    format!(
        "<root><return>0</return><desc>请求成功</desc><token>{}</token>\
         <expiretime>{}</expiretime><accessToken></accessToken></root>",
        token, expire_secs
    )
}

#[tokio::test]
async fn refresh_rebuilds_authorization_and_expiry() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST).path(REFRESH_PATH);
        then.status(200).body(success_body("new-token", 2592000));
    });

    let config = base_config();
    let before = chrono::Utc::now().timestamp_millis();
    let refreshed = auth::refresh_token_at(&config, &server.url(REFRESH_PATH))
        .await
        .expect("refresh should succeed");
    mock.assert();

    let expected = base64::engine::general_purpose::STANDARD.encode("pc:13800138000:new-token");
    assert_eq!(refreshed.authorization, expected);
    assert_eq!(refreshed.refresh_token.as_deref(), Some("new-token"));

    let expire = refreshed
        .token_expire_time
        .expect("expire time should be set");
    let remaining = expire - before;
    // 允许请求耗时造成的正向偏差，也允许 60 秒的时钟粒度。
    assert!(
        remaining > (2592000 - 60) * 1000 && remaining < (2592000 + 60) * 1000,
        "expire time should follow <expiretime>, got {} ms",
        remaining
    );
}

#[tokio::test]
async fn refresh_reports_server_error_code() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST).path(REFRESH_PATH);
        then.status(200).body(
            "<root><return>4006</return><desc>The token does not exist or has expired.</desc>\
             <token></token><accessToken></accessToken></root>",
        );
    });

    let err = auth::refresh_token_at(&base_config(), &server.url(REFRESH_PATH))
        .await
        .expect_err("refresh should fail");
    mock.assert();
    let text = err.to_string();
    assert!(text.contains("4006"), "error should carry code: {}", text);
    assert!(
        text.contains("expired"),
        "error should carry desc: {}",
        text
    );
}

#[tokio::test]
async fn refresh_rejects_empty_token() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path(REFRESH_PATH);
        then.status(200)
            .body("<root><return>0</return><token></token><accessToken></accessToken></root>");
    });

    let err = auth::refresh_token_at(&base_config(), &server.url(REFRESH_PATH))
        .await
        .expect_err("empty token should fail");
    assert!(err.to_string().contains("empty token"), "{}", err);
}

#[tokio::test]
async fn skips_refresh_when_token_is_fresh() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST).path(REFRESH_PATH);
        then.status(200)
            .body(success_body("should-not-be-used", 2592000));
    });

    let mut config = base_config();
    config.token_expire_time = Some(
        chrono::Utc::now().timestamp_millis() + TOKEN_REFRESH_THRESHOLD_MS + 24 * 60 * 60 * 1000,
    );

    let result = auth::refresh_if_needed_at(&config, &server.url(REFRESH_PATH))
        .await
        .expect("should not fail");
    assert!(result.is_none(), "fresh token must not be refreshed");
    mock.assert_calls(0);
}

#[tokio::test]
async fn refreshes_when_token_is_near_expiry() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST).path(REFRESH_PATH);
        then.status(200).body(success_body("new-token", 2592000));
    });

    let mut config = base_config();
    config.token_expire_time = Some(chrono::Utc::now().timestamp_millis() + 24 * 60 * 60 * 1000);

    let result = auth::refresh_if_needed_at(&config, &server.url(REFRESH_PATH))
        .await
        .expect("refresh should succeed");
    mock.assert_calls(1);
    let refreshed = result.expect("near-expiry token should be refreshed");
    assert_eq!(refreshed.refresh_token.as_deref(), Some("new-token"));
}
