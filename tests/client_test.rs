use cloud139::client::{Client, ClientError};
use cloud139::domain::StorageType;
use cloud139::config::Config;

#[test]
fn test_generate_rand_str() {
    let s = cloud139::client::generate_rand_str(10);
    assert_eq!(s.len(), 10);
}

#[test]
fn test_generate_rand_str_empty() {
    let s = cloud139::client::generate_rand_str(0);
    assert_eq!(s.len(), 0);
}

#[test]
fn test_generate_rand_str_charset() {
    let s = cloud139::client::generate_rand_str(1000);
    for c in s.chars() {
        assert!(c.is_ascii_alphanumeric() || c == '-' || c == '_');
    }
}

#[test]
fn test_sort_json_value_object() {
    let value = serde_json::json!({"b": 2, "a": 1});
    let result = cloud139::client::sort_json_value_to_string(&value);
    assert!(result.contains("\"a\""));
    assert!(result.contains("\"b\""));
}

#[test]
fn test_sort_json_value_array() {
    let value = serde_json::json!([3, 1, 2]);
    let result = cloud139::client::sort_json_value_to_string(&value);
    assert!(result.contains("1"));
    assert!(result.contains("2"));
    assert!(result.contains("3"));
}

#[test]
fn test_sort_json_value_string() {
    let value = serde_json::json!("hello");
    let result = cloud139::client::sort_json_value_to_string(&value);
    assert!(result.contains("hello"));
}

#[test]
fn test_sort_json_value_number() {
    let value = serde_json::json!(42);
    let result = cloud139::client::sort_json_value_to_string(&value);
    assert!(result.contains("42"));
}

#[test]
fn test_sort_json_value_bool_true() {
    let value = serde_json::json!(true);
    let result = cloud139::client::sort_json_value_to_string(&value);
    assert!(result.contains("true"));
}

#[test]
fn test_sort_json_value_bool_false() {
    let value = serde_json::json!(false);
    let result = cloud139::client::sort_json_value_to_string(&value);
    assert!(result.contains("false"));
}

#[test]
fn test_sort_json_value_null() {
    let value = serde_json::json!(null);
    let result = cloud139::client::sort_json_value_to_string(&value);
    assert!(result.contains("null"));
}

#[test]
fn test_sort_json_nested() {
    let value = serde_json::json!({"outer": {"inner": 1}, "z": 0});
    let result = cloud139::client::sort_json_value_to_string(&value);
    assert!(result.contains("inner"));
    assert!(result.contains("outer"));
}

#[test]
fn test_storage_type_as_str_personal() {
    assert_eq!(StorageType::PersonalNew.as_str(), "personal_new");
}

#[test]
fn test_storage_type_as_str_family() {
    assert_eq!(StorageType::Family.as_str(), "family");
}

#[test]
fn test_storage_type_as_str_group() {
    assert_eq!(StorageType::Group.as_str(), "group");
}

#[test]
fn test_storage_type_from_str_raw_family() {
    assert_eq!(StorageType::from_str_raw("family"), StorageType::Family);
}

#[test]
fn test_storage_type_from_str_raw_group() {
    assert_eq!(StorageType::from_str_raw("group"), StorageType::Group);
}

#[test]
fn test_storage_type_from_str_raw_default() {
    assert_eq!(StorageType::from_str_raw("other"), StorageType::PersonalNew);
    assert_eq!(StorageType::from_str_raw(""), StorageType::PersonalNew);
}

#[test]
fn test_storage_type_svc_type_personal() {
    assert_eq!(StorageType::PersonalNew.svc_type(), "1");
}

#[test]
fn test_storage_type_svc_type_family() {
    assert_eq!(StorageType::Family.svc_type(), "2");
}

#[test]
fn test_storage_type_svc_type_group() {
    assert_eq!(StorageType::Group.svc_type(), "3");
}

#[test]
fn test_storage_type_default() {
    let st: StorageType = Default::default();
    assert_eq!(st, StorageType::PersonalNew);
}

#[test]
fn test_storage_type_serialize() {
    let st = StorageType::PersonalNew;
    let json = serde_json::to_string(&st).unwrap();
    assert_eq!(json, "\"personalnew\"");
}

#[test]
fn test_storage_type_deserialize() {
    let st: StorageType = serde_json::from_str("\"family\"").unwrap();
    assert_eq!(st, StorageType::Family);
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
fn test_client_new() {
    let config = Config {
        authorization: "test_auth".to_string(),
        account: "test_account".to_string(),
        storage_type: "personal".to_string(),
        ..Default::default()
    };

    let client = Client::new(config.clone());
    assert_eq!(client.config.account, "test_account");
    assert_eq!(client.config.authorization, "test_auth");
}

#[tokio::test]
async fn test_client_login_invalid_token() {
    let result = Client::login("invalid_base64".to_string(), "personal".to_string(), None).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_client_login_invalid_format() {
    let token =
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, "invalid_format");
    let result = Client::login(token, "personal".to_string(), None).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_client_login_missing_parts() {
    let token = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, "pc:account");
    let result = Client::login(token, "personal".to_string(), None).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_client_login_missing_token_parts() {
    let token = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        "pc:account:token",
    );
    let result = Client::login(token, "personal".to_string(), None).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_client_login_valid() {
    let expire_time = chrono::Utc::now().timestamp_millis() + 30 * 24 * 60 * 60 * 1000;
    let token_str = format!("pc:13800138000:token|abc|def|{}", expire_time);
    let token = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &token_str);

    let result = Client::login(token, "personal".to_string(), None).await;
    assert!(result.is_ok());

    let client = result.unwrap();
    assert_eq!(client.config.account, "13800138000");
    assert_eq!(client.config.storage_type, "personal");
}

#[tokio::test]
async fn test_client_login_with_cloud_id() {
    let expire_time = chrono::Utc::now().timestamp_millis() + 30 * 24 * 60 * 60 * 1000;
    let token_str = format!("pc:13800138000:token|abc|def|{}", expire_time);
    let token = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &token_str);

    let result = Client::login(token, "family".to_string(), Some("cloud123".to_string())).await;
    assert!(result.is_ok());

    let client = result.unwrap();
    assert_eq!(client.config.cloud_id, Some("cloud123".to_string()));
}

#[tokio::test]
async fn test_client_refresh_token_invalid_token() {
    let config = Config {
        authorization: "invalid".to_string(),
        account: "test".to_string(),
        storage_type: "personal".to_string(),
        refresh_token: None,
        token_expire_time: None,
        ..Default::default()
    };

    let mut client = Client::new(config);
    let result = client.refresh_token_if_needed().await;
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

#[test]
fn test_storage_type_serialize_new() {
    let st = StorageType::PersonalNew;
    let json = serde_json::to_string(&st).unwrap();
    assert!(json.contains("personalnew"));
}

#[test]
fn test_storage_type_deserialize_new() {
    let st: StorageType = serde_json::from_str("\"family\"").unwrap();
    assert_eq!(st, StorageType::Family);
}

#[test]
fn test_storage_type_default_new() {
    let st = StorageType::default();
    assert_eq!(st, StorageType::PersonalNew);
}
