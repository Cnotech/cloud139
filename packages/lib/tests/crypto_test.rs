use cloud139::utils::rand::generate_rand_str;
use cloud139::utils::*;

#[test]
fn test_md5_hash() {
    assert_eq!(md5_hash(""), "d41d8cd98f00b204e9800998ecf8427e");
    assert_eq!(md5_hash("hello"), "5d41402abc4b2a76b9719d911017c592");
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
    assert_eq!(encode_uri_component("!'()*"), "!'()*");
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
fn signature_matches_upstream_go_vectors_with_movie_names() {
    // Generated with AList/OpenList encodeURIComponent + calSign, including the
    // punctuation that previously produced different signatures in Rust.
    let stamp = "2026-11-18 12:00:00";
    let nonce = "0123456789abcdef";
    for (body, expected) in [
        (
            r#"{"name":"电影 (2026)!'*.bin","size":8589934592}"#,
            "2C9FA221DC7111889F42AD8109CBA963",
        ),
        (
            r#"{"name":"a&b+c.bin","parentFileId":"/"}"#,
            "51F6CCB395C487EDEAD3057842330C4B",
        ),
        (
            r#"{"name":"普通中文.bin","size":10737418240}"#,
            "691BD5B14771E78E6C5B9048509E1B2E",
        ),
    ] {
        assert_eq!(calc_sign(body, stamp, nonce), expected);
    }
    assert_eq!(
        encode_uri_component("电影 (2026)!'*.bin"),
        "%E7%94%B5%E5%BD%B1%20(2026)!'*.bin"
    );
}

#[test]
fn test_generate_random_string_length() {
    let s5 = generate_rand_str(5);
    let s10 = generate_rand_str(10);
    let s100 = generate_rand_str(100);

    assert_eq!(s5.len(), 5);
    assert_eq!(s10.len(), 10);
    assert_eq!(s100.len(), 100);
}

#[test]
fn test_generate_random_string_charset() {
    let s = generate_rand_str(1000);
    for c in s.chars() {
        assert!(c.is_ascii_alphanumeric() || c == '-' || c == '_');
    }
}

#[test]
fn test_calc_file_sha256_nonexistent() {
    let result = calc_file_sha256("/nonexistent/file/path");
    assert!(result.is_err());
}
