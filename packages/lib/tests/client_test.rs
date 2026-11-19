use cloud139::client::{ClientError, auth};

#[test]
fn test_generate_rand_str() {
    let s = cloud139::utils::generate_rand_str(10);
    assert_eq!(s.len(), 10);
}

#[test]
fn test_generate_rand_str_empty() {
    let s = cloud139::utils::generate_rand_str(0);
    assert_eq!(s.len(), 0);
}

#[test]
fn test_generate_rand_str_charset() {
    let s = cloud139::utils::generate_rand_str(1000);
    for c in s.chars() {
        assert!(c.is_ascii_alphanumeric() || c == '-' || c == '_');
    }
}

#[test]
fn test_sort_json_value_object() {
    let value = serde_json::json!({"b": 2, "a": 1});
    let result = cloud139::utils::sort_json_value_to_string(&value);
    assert!(result.contains("\"a\""));
    assert!(result.contains("\"b\""));
}

#[test]
fn test_sort_json_value_array() {
    let value = serde_json::json!([3, 1, 2]);
    let result = cloud139::utils::sort_json_value_to_string(&value);
    assert!(result.contains("1"));
    assert!(result.contains("2"));
    assert!(result.contains("3"));
}

#[test]
fn test_sort_json_value_string() {
    let value = serde_json::json!("hello");
    let result = cloud139::utils::sort_json_value_to_string(&value);
    assert!(result.contains("hello"));
}

#[test]
fn test_sort_json_value_number() {
    let value = serde_json::json!(42);
    let result = cloud139::utils::sort_json_value_to_string(&value);
    assert!(result.contains("42"));
}

#[test]
fn test_sort_json_value_bool_true() {
    let value = serde_json::json!(true);
    let result = cloud139::utils::sort_json_value_to_string(&value);
    assert!(result.contains("true"));
}

#[test]
fn test_sort_json_value_bool_false() {
    let value = serde_json::json!(false);
    let result = cloud139::utils::sort_json_value_to_string(&value);
    assert!(result.contains("false"));
}

#[test]
fn test_sort_json_value_null() {
    let value = serde_json::json!(null);
    let result = cloud139::utils::sort_json_value_to_string(&value);
    assert!(result.contains("null"));
}

#[test]
fn test_sort_json_nested() {
    let value = serde_json::json!({"outer": {"inner": 1}, "z": 0});
    let result = cloud139::utils::sort_json_value_to_string(&value);
    assert!(result.contains("inner"));
    assert!(result.contains("outer"));
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
    let result = auth::parse_login_token(&token);
    assert!(result.is_err());
}

#[test]
fn test_client_error_from_reqwest() {
    let err: ClientError = std::io::Error::new(std::io::ErrorKind::NotFound, "test").into();
    assert!(matches!(err, ClientError::Io(_)));
}

#[test]
fn test_client_error_from_config() {
    use cloud139::config::ConfigError;
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
