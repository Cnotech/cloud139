use cloud139::utils::*;

#[test]
fn test_sha1_hash() {
    assert_eq!(sha1_hash(""), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
    assert_eq!(
        sha1_hash("hello"),
        "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d"
    );
    assert_eq!(
        sha1_hash("hello world"),
        "2aae6c35c94fcfb415dbe95f408b9ce91ee846ed"
    );
}

#[test]
fn test_md5_hash() {
    assert_eq!(md5_hash(""), "d41d8cd98f00b204e9800998ecf8427e");
    assert_eq!(md5_hash("hello"), "5d41402abc4b2a76b9719d911017c592");
}

#[test]
fn test_aes_cbc_encrypt_decrypt() {
    let key = b"1234567890abcdef";
    let iv = b"0000000000000000";

    let plaintext = b"Hello, World!";
    let ciphertext = aes_cbc_encrypt(plaintext, key, iv).unwrap();
    let decrypted = aes_cbc_decrypt(&ciphertext, key, iv).unwrap();

    assert_eq!(&decrypted, plaintext);
}

#[test]
fn test_aes_cbc_encrypt_multiple_blocks() {
    let key = b"1234567890abcdef";
    let iv = b"0000000000000000";

    let plaintext = b"This is a longer message that exceeds one block size of 16 bytes!";
    let ciphertext = aes_cbc_encrypt(plaintext, key, iv).unwrap();
    let decrypted = aes_cbc_decrypt(&ciphertext, key, iv).unwrap();

    assert_eq!(&decrypted, plaintext);
}

#[test]
fn test_aes_cbc_encrypt_exact_block() {
    let key = b"1234567890abcdef";
    let iv = b"0000000000000000";

    let plaintext = b"1234567890123456";
    let ciphertext = aes_cbc_encrypt(plaintext, key, iv).unwrap();
    let decrypted = aes_cbc_decrypt(&ciphertext, key, iv).unwrap();

    assert_eq!(&decrypted, plaintext);
}

#[test]
fn test_aes_ecb_decrypt_invalid_length() {
    let key = b"1234567890abcdef";
    let result = aes_ecb_decrypt(b"short", key);
    assert!(result.is_err());
}

#[test]
fn test_pkcs7_pad() {
    assert_eq!(
        pkcs7_pad(b"Hello", 16),
        b"Hello\x0b\x0b\x0b\x0b\x0b\x0b\x0b\x0b\x0b\x0b\x0b"
    );
    assert_eq!(
        pkcs7_pad(b"1234567890123456", 16),
        b"1234567890123456\x10\x10\x10\x10\x10\x10\x10\x10\x10\x10\x10\x10\x10\x10\x10\x10"
    );
}

#[test]
fn test_pkcs7_unpad() {
    let padded = pkcs7_pad(b"Hello", 16);
    let unpadded = pkcs7_unpad(&padded).unwrap();
    assert_eq!(unpadded, b"Hello");
}

#[test]
fn test_pkcs7_unpad_empty() {
    let result = pkcs7_unpad(b"");
    assert!(result.is_err());
}

#[test]
fn test_pkcs7_unpad_invalid_padding() {
    let result = pkcs7_unpad(b"Hello\x01\x02\x03");
    assert!(result.is_err());
}

#[test]
fn test_encode_uri_component() {
    assert_eq!(encode_uri_component("hello"), "hello");
    assert_eq!(encode_uri_component("hello world"), "hello%20world");
    assert_eq!(encode_uri_component("!"), "%21");
    assert_eq!(encode_uri_component("'"), "%27");
    assert_eq!(encode_uri_component("()"), "%28%29");
    assert_eq!(encode_uri_component("*"), "%2A");
    assert_eq!(encode_uri_component("a/b"), "a%2Fb");
    assert_eq!(encode_uri_component("a?b"), "a%3Fb");
    assert_eq!(encode_uri_component("a#b"), "a%23b");
    assert_eq!(encode_uri_component("中文"), "%E4%B8%AD%E6%96%87");
}

#[test]
fn test_calc_sign() {
    let body = r#"{"key":"value"}"#;
    let ts = "2024-01-01 00:00:00";
    let rand_str = "abcd1234";

    let sign = calc_sign(body, ts, rand_str);
    assert_eq!(sign.len(), 32);
    assert!(sign.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn test_calc_sign_different_bodies() {
    let ts = "2024-01-01 00:00:00";
    let rand_str = "abcd1234";

    let sign1 = calc_sign(r#"{"a":1}"#, ts, rand_str);
    let sign2 = calc_sign(r#"{"b":2}"#, ts, rand_str);

    assert_ne!(sign1, sign2);
}

#[test]
fn test_generate_random_string_length() {
    let s5 = generate_random_string(5);
    let s10 = generate_random_string(10);
    let s100 = generate_random_string(100);

    assert_eq!(s5.len(), 5);
    assert_eq!(s10.len(), 10);
    assert_eq!(s100.len(), 100);
}

#[test]
fn test_generate_random_string_charset() {
    let s = generate_random_string(1000);
    for c in s.chars() {
        assert!(c.is_ascii_alphanumeric() || c == '-' || c == '_');
    }
}

#[test]
fn test_calc_file_hash_nonexistent() {
    let result = calc_file_hash("/nonexistent/file/path");
    assert!(result.is_err());
}

#[test]
fn test_calc_file_sha256_nonexistent() {
    let result = calc_file_sha256("/nonexistent/file/path");
    assert!(result.is_err());
}

#[test]
fn test_aes_ecb_decrypt_non_multiple_block_size() {
    let key = b"0123456789abcdef";
    let bad_ciphertext = vec![0u8; 15];
    let result = aes_ecb_decrypt(&bad_ciphertext, key);
    assert!(
        result.is_err(),
        "应当返回 Err，因为密文长度不是块大小的倍数"
    );
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("15"), "错误信息应包含实际长度 15");
}

#[test]
fn test_aes_cbc_roundtrip() {
    let key = b"0123456789abcdef";
    let iv = b"abcdef0123456789";
    let plaintext = b"Hello, cloud139!";
    let encrypted = aes_cbc_encrypt(plaintext, key, iv).expect("加密失败");
    let decrypted = aes_cbc_decrypt(&encrypted, key, iv).expect("解密失败");
    assert_eq!(decrypted, plaintext, "解密后应还原原始数据");
}
