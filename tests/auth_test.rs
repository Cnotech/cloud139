use cloud139::config::Config;

#[tokio::test]
async fn test_login_invalid_base64() {
    let result = cloud139::client::auth::login("!!!invalid!!!", "personal", None).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_login_invalid_utf8() {
    let invalid_utf8 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &[0x80, 0x81, 0x82]);
    let result = cloud139::client::auth::login(&invalid_utf8, "personal", None).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_login_missing_parts() {
    let token = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, "pc:account");
    let result = cloud139::client::auth::login(&token, "personal", None).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_login_missing_token_info_parts() {
    let token = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, "pc:account:token");
    let result = cloud139::client::auth::login(&token, "personal", None).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_login_invalid_expire_time() {
    let token_str = "pc:account:token|abc|def|invalid";
    let token = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, token_str);
    let result = cloud139::client::auth::login(&token, "personal", None).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_login_valid() {
    let expire_time = chrono::Utc::now().timestamp_millis() + 30 * 24 * 60 * 60 * 1000;
    let token_str = format!("pc:13800138000:token|abc|def|{}", expire_time);
    let token = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &token_str);
    
    let result = cloud139::client::auth::login(&token, "family", Some("cloud123")).await;
    assert!(result.is_ok());
    
    let config = result.unwrap();
    assert_eq!(config.account, "13800138000");
    assert_eq!(config.storage_type, "family");
    assert_eq!(config.cloud_id, Some("cloud123".to_string()));
    assert!(config.refresh_token.is_some());
    assert!(config.token_expire_time.is_some());
}

#[tokio::test]
async fn test_login_no_cloud_id() {
    let expire_time = chrono::Utc::now().timestamp_millis() + 30 * 24 * 60 * 60 * 1000;
    let token_str = format!("pc:13800138000:token|abc|def|{}", expire_time);
    let token = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &token_str);
    
    let result = cloud139::client::auth::login(&token, "group", None).await;
    assert!(result.is_ok());
    
    let config = result.unwrap();
    assert_eq!(config.cloud_id, None);
}

#[test]
fn test_get_account() {
    let config = Config {
        account: "13800138000".to_string(),
        ..Default::default()
    };
    
    let account = cloud139::client::auth::get_account(&config);
    assert_eq!(account, "13800138000");
}

#[test]
fn test_get_account_empty() {
    let config = Config::default();
    let account = cloud139::client::auth::get_account(&config);
    assert_eq!(account, "");
}
