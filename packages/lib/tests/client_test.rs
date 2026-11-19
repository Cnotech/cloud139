use cloud139::{client::auth::token::parse_login_token, error::ClientError};

#[test]
fn test_generate_rand_str() {
    let s = cloud139::utils::crypto::generate_rand_str(10);
    assert_eq!(s.len(), 10);
}

#[test]
fn test_generate_rand_str_empty() {
    let s = cloud139::utils::crypto::generate_rand_str(0);
    assert_eq!(s.len(), 0);
}

#[test]
fn test_generate_rand_str_charset() {
    let s = cloud139::utils::crypto::generate_rand_str(1000);
    for c in s.chars() {
        assert!(c.is_ascii_alphanumeric());
    }
}

#[test]
fn test_client_error_display() {
    let err = ClientError::NotLoggedIn;
    assert_eq!(err.to_string(), "未登录");

    let err = ClientError::TokenExpired;
    assert_eq!(err.to_string(), "Token 已过期");

    let err = ClientError::Other("test error".to_string());
    assert_eq!(err.to_string(), "其他错误: test error");
}

#[test]
fn test_login_invalid_format() {
    let token =
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, "invalid_format");
    let result = parse_login_token(&token);
    assert!(result.is_err());
}

#[test]
fn test_client_error_from_reqwest() {
    let err: ClientError = std::io::Error::new(std::io::ErrorKind::NotFound, "test").into();
    assert!(matches!(err, ClientError::Io(_)));
}

#[test]
fn test_client_error_from_config() {
    use cloud139::error::ConfigError;
    let err: ClientError = ConfigError::NotFound.into();
    assert!(matches!(err, ClientError::Config(ConfigError::NotFound)));
}

#[test]
fn test_client_error_from_json() {
    let err: ClientError = serde_json::from_str::<serde_json::Value>("invalid")
        .unwrap_err()
        .into();
    assert!(matches!(err, ClientError::Json(_)));
}

#[test]
fn test_client_error_other_new() {
    let err = ClientError::Other("custom error".to_string());
    assert_eq!(err.to_string(), "其他错误: custom error");
}
