#![allow(dead_code)]

mod utils_test_extended {
    use cloud139_core::utils::crypto::{
        aes_cbc_decrypt, aes_cbc_encrypt, calc_file_hash, calc_file_sha256, calc_sign,
        encode_uri_component, md5_hash, pkcs7_pad, sha1_hash,
    };
    use cloud139_core::utils::rand::generate_rand_str;
    use cloud139_core::utils::width::{pad_with_width, str_width, truncate_with_width};
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_sha1_hash() {
        let result = sha1_hash("hello");
        assert_eq!(result, "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d");
    }

    #[test]
    fn test_sha1_hash_empty() {
        let result = sha1_hash("");
        assert_eq!(result, "da39a3ee5e6b4b0d3255bfef95601890afd80709");
    }

    #[test]
    fn test_sha1_hash_long_string() {
        let input = "a".repeat(1000);
        let result = sha1_hash(&input);
        assert_eq!(result.len(), 40);
    }

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
    fn test_aes_cbc_encrypt_decrypt_roundtrip() {
        let key = b"1234567890123456";
        let iv = b"1234567890123456";
        let plaintext = b"Hello, World!";

        let ciphertext = aes_cbc_encrypt(plaintext, key, iv).unwrap();
        let decrypted = aes_cbc_decrypt(&ciphertext, key, iv).unwrap();

        assert_eq!(&decrypted, plaintext);
    }

    #[test]
    fn test_aes_cbc_encrypt_empty() {
        let key = b"1234567890123456";
        let iv = b"1234567890123456";
        let plaintext = b"";

        let ciphertext = aes_cbc_encrypt(plaintext, key, iv).unwrap();
        let decrypted = aes_cbc_decrypt(&ciphertext, key, iv).unwrap();

        assert_eq!(&decrypted, plaintext);
    }

    #[test]
    fn test_aes_cbc_encrypt_16_bytes() {
        let key = b"1234567890123456";
        let iv = b"1234567890123456";
        let plaintext = b"1234567890123456";

        let ciphertext = aes_cbc_encrypt(plaintext, key, iv).unwrap();
        let decrypted = aes_cbc_decrypt(&ciphertext, key, iv).unwrap();

        assert_eq!(&decrypted, plaintext);
    }

    #[test]
    fn test_aes_cbc_encrypt_32_bytes() {
        let key = b"1234567890123456";
        let iv = b"1234567890123456";
        let plaintext = b"12345678901234567890123456789012";

        let ciphertext = aes_cbc_encrypt(plaintext, key, iv).unwrap();
        let decrypted = aes_cbc_decrypt(&ciphertext, key, iv).unwrap();

        assert_eq!(&decrypted, plaintext);
    }

    #[test]
    fn test_aes_cbc_encrypt_longer() {
        let key = b"1234567890123456";
        let iv = b"1234567890123456";
        let plaintext = b"Lorem ipsum dolor sit amet, consectetur adipiscing elit.";

        let ciphertext = aes_cbc_encrypt(plaintext, key, iv).unwrap();
        let decrypted = aes_cbc_decrypt(&ciphertext, key, iv).unwrap();

        assert_eq!(&decrypted, plaintext);
    }

    #[test]
    fn test_pkcs7_pad_16_bytes() {
        // For 16 bytes, PKCS7 adds a full block of padding
        let data = b"1234567890123456";
        let result = pkcs7_pad(data, 16);
        assert_eq!(result.len(), 32);
    }

    #[test]
    fn test_pkcs7_pad_15_bytes() {
        // For 15 bytes, PKCS7 adds 1 byte of padding to reach 16
        let data = b"123456789012345";
        let result = pkcs7_pad(data, 16);
        assert_eq!(result.len(), 16);
    }

    #[test]
    fn test_pkcs7_pad_1_byte() {
        // For 1 byte, PKCS7 adds 15 bytes of padding
        let data = b"1";
        let result = pkcs7_pad(data, 16);
        assert_eq!(result.len(), 16);
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
    fn test_calc_file_hash() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "hello").unwrap();

        let result = calc_file_hash(file_path.to_str().unwrap()).unwrap();
        assert_eq!(result, "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d");
    }

    #[test]
    fn test_calc_file_hash_not_found() {
        let result = calc_file_hash("/nonexistent/file.txt");
        assert!(result.is_err());
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

    #[test]
    fn test_str_width_ascii() {
        assert_eq!(str_width("hello"), 5);
        assert_eq!(str_width("Hello World"), 11);
    }

    #[test]
    fn test_str_width_unicode() {
        assert_eq!(str_width("你好"), 4);
        assert_eq!(str_width("hello世界"), 9);
    }

    #[test]
    fn test_str_width_empty() {
        assert_eq!(str_width(""), 0);
    }

    #[test]
    fn test_str_width_mixed() {
        assert_eq!(str_width("a你b好c"), 7);
    }

    #[test]
    fn test_truncate_with_width_no_truncate() {
        let result = truncate_with_width("hello", 10);
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_truncate_with_width_exact() {
        let result = truncate_with_width("hello", 5);
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_truncate_with_width_truncate() {
        let result = truncate_with_width("hello world", 5);
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_truncate_with_width_unicode() {
        let result = truncate_with_width("你好世界", 4);
        assert_eq!(result, "你好");
    }

    #[test]
    fn test_truncate_with_width_unicode_partial() {
        let result = truncate_with_width("你好世界", 3);
        assert_eq!(result, "你");
    }

    #[test]
    fn test_truncate_with_width_zero() {
        let result = truncate_with_width("hello", 0);
        assert!(result.is_empty());
    }

    #[test]
    fn test_pad_with_width_no_padding() {
        let result = pad_with_width("hello", 5);
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_pad_with_width_with_padding() {
        let result = pad_with_width("hello", 10);
        assert_eq!(result, "hello     ");
    }

    #[test]
    fn test_pad_with_width_unicode() {
        let result = pad_with_width("你好", 6);
        assert_eq!(result, "你好  ");
    }

    #[test]
    fn test_pad_with_width_truncate() {
        let result = pad_with_width("hello world", 5);
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_pad_with_width_unicode_truncate() {
        let result = pad_with_width("你好世界", 4);
        assert_eq!(result, "你好");
    }

    #[test]
    fn test_pad_with_width_zero() {
        let result = pad_with_width("hello", 0);
        assert!(result.is_empty());
    }
}
