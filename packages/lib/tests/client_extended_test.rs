#![allow(dead_code)]

mod client_test_extended {
    use cloud139::client::ClientError;

    use cloud139::utils::{generate_rand_str, sort_json_value_to_string};
    use serde_json::json;

    #[test]
    fn test_generate_rand_str_length() {
        let result = generate_rand_str(8);
        assert_eq!(result.len(), 8);
    }

    #[test]
    fn test_generate_rand_str_different_lengths() {
        assert_eq!(generate_rand_str(0).len(), 0);
        assert_eq!(generate_rand_str(1).len(), 1);
        assert_eq!(generate_rand_str(16).len(), 16);
        assert_eq!(generate_rand_str(32).len(), 32);
    }

    #[test]
    fn test_generate_rand_str_alphanumeric() {
        let result = generate_rand_str(100);
        for c in result.chars() {
            assert!(c.is_ascii_alphanumeric());
        }
    }

    #[test]
    fn test_generate_rand_str_randomness() {
        let results: Vec<String> = (0..100).map(|_| generate_rand_str(8)).collect();
        let unique: std::collections::HashSet<_> = results.into_iter().collect();
        assert!(unique.len() > 90);
    }

    #[test]
    fn test_sort_json_value_to_string_empty_object() {
        let value = json!({});
        let result = sort_json_value_to_string(&value);
        assert!(result.contains("{}"));
    }

    #[test]
    fn test_sort_json_value_to_string_single_key() {
        let value = json!({"b": 1});
        let result = sort_json_value_to_string(&value);
        assert!(result.contains("\"b\""));
    }

    #[test]
    fn test_sort_json_value_to_string_multiple_keys() {
        let value = json!({"b": 1, "a": 2, "c": 3});
        let result = sort_json_value_to_string(&value);
        let a_pos = result.find("\"a\"").unwrap_or(0);
        let b_pos = result.find("\"b\"").unwrap_or(0);
        let c_pos = result.find("\"c\"").unwrap_or(0);
        assert!(a_pos < b_pos && b_pos < c_pos);
    }

    #[test]
    fn test_sort_json_value_to_string_nested_object() {
        let value = json!({"b": {"y": 1, "x": 2}, "a": 1});
        let result = sort_json_value_to_string(&value);
        assert!(result.contains("\"a\""));
        assert!(result.contains("\"b\""));
        assert!(result.contains("\"x\""));
        assert!(result.contains("\"y\""));
    }

    #[test]
    fn test_sort_json_value_to_string_array() {
        let value = json!([3, 1, 2]);
        let result = sort_json_value_to_string(&value);
        assert!(result.contains("["));
        assert!(result.contains("]"));
    }

    #[test]
    fn test_sort_json_value_to_string_string() {
        let value = json!("hello");
        let result = sort_json_value_to_string(&value);
        assert!(result.contains("\"hello\""));
    }

    #[test]
    fn test_sort_json_value_to_string_number() {
        let value = json!(42);
        let result = sort_json_value_to_string(&value);
        assert!(result.contains("42"));
    }

    #[test]
    fn test_sort_json_value_to_string_boolean() {
        let value = json!(true);
        let result = sort_json_value_to_string(&value);
        assert!(result.contains("true"));
    }

    #[test]
    fn test_sort_json_value_to_string_null() {
        let value = json!(null);
        let result = sort_json_value_to_string(&value);
        assert!(result.contains("null"));
    }

    #[test]
    fn test_sort_json_value_complex() {
        let value = json!({
            "name": "test",
            "age": 30,
            "items": ["a", "b", "c"],
            "nested": {
                "z": 1,
                "a": 2
            }
        });
        let result = sort_json_value_to_string(&value);
        assert!(result.contains("\"age\""));
        assert!(result.contains("\"items\""));
        assert!(result.contains("\"name\""));
        assert!(result.contains("\"nested\""));
    }

    #[test]
    fn test_client_error_display() {
        let err = ClientError::NotLoggedIn;
        assert_eq!(err.to_string(), "未登录");

        let err = ClientError::TokenExpired;
        assert_eq!(err.to_string(), "Token 已过期");
    }

    #[test]
    fn test_client_error_from_json() {
        let err: ClientError = serde_json::from_str::<serde_json::Value>("invalid")
            .unwrap_err()
            .into();
        assert!(matches!(err, ClientError::Json(_)));
    }

    #[test]
    fn test_client_error_other() {
        let err = ClientError::Other("custom error".to_string());
        assert!(err.to_string().contains("custom error"));
    }

    #[test]
    fn test_client_error_debug() {
        let err = ClientError::NotLoggedIn;
        let debug_str = format!("{:?}", err);
        assert!(!debug_str.is_empty());
    }
}
