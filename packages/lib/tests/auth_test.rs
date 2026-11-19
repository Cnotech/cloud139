use cloud139::client::auth::token::parse_login_token;

#[test]
fn test_login_invalid_base64() {
    let result = parse_login_token("!!!invalid!!!");
    assert!(result.is_err());
}

#[test]
fn test_login_invalid_utf8() {
    let invalid_utf8 = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        [0x80, 0x81, 0x82],
    );
    let result = parse_login_token(&invalid_utf8);
    assert!(result.is_err());
}

#[test]
fn test_login_missing_parts() {
    let token = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, "pc:account");
    let result = parse_login_token(&token);
    assert!(result.is_err());
}

#[test]
fn test_login_missing_token_info_parts() {
    let token = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        "pc:account:token",
    );
    let result = parse_login_token(&token);
    assert!(result.is_err());
}

#[test]
fn test_login_invalid_expire_time() {
    let token_str = "pc:account:token|abc|def|invalid";
    let token = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, token_str);
    let result = parse_login_token(&token);
    assert!(result.is_err());
}

#[test]
fn test_login_valid() {
    let expire_time = chrono::Utc::now().timestamp_millis() + 30 * 24 * 60 * 60 * 1000;
    let token_str = format!("pc:13800138000:token|abc|def|{}", expire_time);
    let token = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &token_str);

    let result = parse_login_token(&format!("Basic {token}"));
    assert!(result.is_ok());

    let config = result.unwrap();
    assert_eq!(config.authorization, token);
    assert_eq!(config.account, "13800138000");
    assert_eq!(config.storage_type, "personal_new");
    assert!(config.refresh_token.is_some());
    assert!(config.token_expire_time.is_some());
}
