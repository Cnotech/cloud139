#![allow(dead_code)]

mod client_test_extended {
    use cloud139::error::ClientError;

    use cloud139::utils::rand::generate_rand_str;

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
