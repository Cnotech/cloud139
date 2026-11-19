#![allow(dead_code)]

mod utils_test_extended {
    use cloud139::utils::crypto::{
        calc_file_sha256, calc_sign, encode_uri_component, generate_rand_str, md5_hash,
    };
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_md5_hash() {
        let result = md5_hash("hello");
        assert_eq!(result, "5d41402abc4b2a76b9719d911017c592");
    }

    #[test]
    fn test_md5_hash_empty() {
        let result = md5_hash("");
        assert_eq!(result, "d41d8cd98f00b204e9800998ecf8427e");
    }

    #[test]
    fn test_md5_hash_long_string() {
        let input = "test".repeat(100);
        let result = md5_hash(&input);
        assert_eq!(result.len(), 32);
    }

    #[test]
    fn test_encode_uri_component_alphanumeric() {
        assert_eq!(encode_uri_component("abc123"), "abc123");
    }

    #[test]
    fn test_encode_uri_component_space() {
        assert_eq!(encode_uri_component("hello world"), "hello%20world");
    }

    #[test]
    fn test_encode_uri_component_special_chars() {
        let result = encode_uri_component("!()*'");
        assert_eq!(result, "!()*'");
    }

    #[test]
    fn test_encode_uri_component_unicode() {
        let result = encode_uri_component("你好");
        assert!(result.contains("%E4"));
    }

    #[test]
    fn test_encode_uri_component_mixed() {
        let result = encode_uri_component("hello世界123");
        assert!(result.contains("hello"));
        assert!(result.contains("123"));
        assert!(result.contains("%E4%B8%96"));
    }

    #[test]
    fn test_encode_uri_component_unreserved() {
        assert_eq!(encode_uri_component("-_.~"), "-_.~");
    }

    #[test]
    fn test_calc_sign() {
        let body = r#"{"test":"value"}"#;
        let ts = "2024-01-01 00:00:00";
        let rand_str = "abcdef123456";

        let result = calc_sign(body, ts, rand_str);
        assert_eq!(result.len(), 32);
    }

    #[test]
    fn test_calc_sign_empty_body() {
        let body = "";
        let ts = "2024-01-01 00:00:00";
        let rand_str = "abcdef123456";

        let result = calc_sign(body, ts, rand_str);
        assert_eq!(result.len(), 32);
    }

    #[test]
    fn test_calc_sign_long_body() {
        let body = "test".repeat(100);
        let ts = "2024-01-01 00:00:00";
        let rand_str = "abcdef123456";

        let result = calc_sign(&body, ts, rand_str);
        assert_eq!(result.len(), 32);
    }

    #[test]
    fn test_generate_random_string_length() {
        let result = generate_rand_str(16);
        assert_eq!(result.len(), 16);
    }

    #[test]
    fn test_generate_random_string_zero() {
        let result = generate_rand_str(0);
        assert!(result.is_empty());
    }

    #[test]
    fn test_generate_random_string_alphanumeric() {
        let result = generate_rand_str(100);
        for c in result.chars() {
            assert!(c.is_ascii_alphanumeric());
        }
    }

    #[test]
    fn test_generate_random_string_randomness() {
        let results: Vec<String> = (0..100).map(|_| generate_rand_str(8)).collect();
        let unique: std::collections::HashSet<_> = results.into_iter().collect();
        assert!(unique.len() > 90);
    }

    #[test]
    fn test_calc_file_sha256() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "hello").unwrap();

        let result = calc_file_sha256(file_path.to_str().unwrap()).unwrap();
        assert_eq!(
            result,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }
}
