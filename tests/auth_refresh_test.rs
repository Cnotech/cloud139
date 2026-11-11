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
        when.method(POST)
            .path(REFRESH_PATH)
            .header("Content-Type", "application/xml;charset=UTF-8")
            .header("Referer", "https://yun.139.com/")
            .body("<root><token>old-refresh-token</token><account>13800138000</account><clienttype>656</clienttype></root>");
        // 使用非默认有效期，确保实际读取接口返回值。
        then.status(200).body(success_body("new-token", 7200));
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
    assert!(remaining >= 7200 * 1000);
    assert!(expire <= chrono::Utc::now().timestamp_millis() + 7200 * 1000);
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

#[tokio::test]
async fn refresh_prefers_token_over_access_token() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST).path(REFRESH_PATH);
        then.status(200).body(
            "<root><return>0</return><token>primary-token</token>\
             <accessToken>fallback-token</accessToken><expiretime>3600</expiretime></root>",
        );
    });
    let config = auth::refresh_token_at(&base_config(), &server.url(REFRESH_PATH))
        .await
        .unwrap();
    mock.assert_calls(1);
    assert_eq!(config.refresh_token.as_deref(), Some("primary-token"));
    assert_eq!(
        config.authorization,
        base64::engine::general_purpose::STANDARD.encode("pc:13800138000:primary-token")
    );
}

#[tokio::test]
async fn refresh_uses_access_token_when_token_is_missing_or_empty() {
    for token_element in ["", "<token></token>"] {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST).path(REFRESH_PATH);
            then.status(200).body(format!(
                "<root><return>0</return>{token_element}\
                 <accessToken>fallback-token</accessToken></root>"
            ));
        });
        let config = auth::refresh_token_at(&base_config(), &server.url(REFRESH_PATH))
            .await
            .unwrap();
        mock.assert_calls(1);
        assert_eq!(config.refresh_token.as_deref(), Some("fallback-token"));
        assert_eq!(
            config.authorization,
            base64::engine::general_purpose::STANDARD.encode("pc:13800138000:fallback-token")
        );
    }
}

#[tokio::test]
async fn refresh_defaults_to_thirty_days_without_expiretime() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST).path(REFRESH_PATH);
        then.status(200)
            .body("<root><return>0</return><token>new-token</token></root>");
    });
    let before = chrono::Utc::now().timestamp_millis();
    let config = auth::refresh_token_at(&base_config(), &server.url(REFRESH_PATH))
        .await
        .unwrap();
    let after = chrono::Utc::now().timestamp_millis();
    mock.assert_calls(1);
    let expiry = config.token_expire_time.unwrap();
    let lifetime = 30 * 86_400_000;
    assert!((before + lifetime..=after + lifetime).contains(&expiry));
}

#[tokio::test]
async fn refresh_without_refresh_token_fails_without_request() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST).path(REFRESH_PATH);
        then.status(200).body(success_body("unexpected", 2592000));
    });
    let mut config = base_config();
    config.refresh_token = None;
    let error = auth::refresh_token_at(&config, &server.url(REFRESH_PATH))
        .await
        .unwrap_err();
    assert!(matches!(error, cloud139::client::ClientError::TokenExpired));
    mock.assert_calls(0);
}

#[tokio::test]
async fn refresh_rejects_malformed_responses() {
    for body in [
        "not XML",
        "<root><token>missing-return-code</token></root>",
        "<root><return>0</return><token>new-token</token><expiretime>invalid</expiretime></root>",
    ] {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST).path(REFRESH_PATH);
            then.status(200).body(body);
        });
        let error = auth::refresh_token_at(&base_config(), &server.url(REFRESH_PATH))
            .await
            .unwrap_err();
        mock.assert_calls(1);
        assert!(
            error
                .to_string()
                .contains("Failed to parse refresh response")
        );
    }
}

#[tokio::test]
async fn refresh_if_needed_recovers_expired_and_unknown_expiry() {
    for expiry in [Some(chrono::Utc::now().timestamp_millis() - 1), None] {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST).path(REFRESH_PATH);
            then.status(200).body(success_body("new-token", 2592000));
        });
        let mut config = base_config();
        config.token_expire_time = expiry;
        let refreshed = auth::refresh_if_needed_at(&config, &server.url(REFRESH_PATH))
            .await
            .unwrap()
            .unwrap();
        mock.assert_calls(1);
        assert_eq!(refreshed.refresh_token.as_deref(), Some("new-token"));
        assert!(!refreshed.needs_refresh());
    }
}

#[tokio::test]
async fn refreshed_authorization_can_be_parsed_by_login() {
    // 全部为模拟凭据，结构与 login -t 接收的 token 相同。
    let old_token = "old-id|1|RCS|1800000000000|old-signature";
    let new_token = "new-id|1|RCS|1900000000000|new-signature";
    let original_authorization =
        base64::engine::general_purpose::STANDARD.encode(format!("pc:13800138000:{old_token}"));
    let mut config = auth::login(&original_authorization, "family", Some("family-id"))
        .await
        .unwrap();
    config.use_large_thumbnail = true;
    config.personal_cloud_host = Some("https://example.invalid".into());
    config.root_folder_id = Some("root-id".into());
    config.user_domain_id = Some("domain-id".into());
    config.custom_upload_part_size = 8 * 1024 * 1024;
    config.report_real_size = false;
    let original = toml::to_string(&config).unwrap();

    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST).path(REFRESH_PATH).body(format!(
            "<root><token>{old_token}</token><account>13800138000</account><clienttype>656</clienttype></root>"
        ));
        then.status(200).body(success_body(new_token, 2592000));
    });
    let refreshed = auth::refresh_token_at(&config, &server.url(REFRESH_PATH))
        .await
        .unwrap();
    mock.assert_calls(1);
    let parsed = auth::login(&refreshed.authorization, "family", Some("family-id"))
        .await
        .unwrap();
    assert_eq!(parsed.account, config.account);
    assert_eq!(parsed.refresh_token.as_deref(), Some(new_token));
    assert_eq!(parsed.token_expire_time, Some(1_900_000_000_000));
    assert_eq!(toml::to_string(&config).unwrap(), original);
    // 除认证的三个字段以外，刷新不改变配置中的其他设置。
    let mut expected = config.clone();
    expected.authorization = refreshed.authorization.clone();
    expected.refresh_token = Some(new_token.into());
    expected.token_expire_time = refreshed.token_expire_time;
    assert_eq!(
        toml::to_string(&refreshed).unwrap(),
        toml::to_string(&expected).unwrap()
    );
}
