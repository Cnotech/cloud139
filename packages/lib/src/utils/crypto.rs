use sha2::{Digest, Sha256};

pub(crate) fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub fn md5_hash(data: &str) -> String {
    let mut hasher = md5::Md5::new();
    hasher.update(data.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn encode_uri_component(s: &str) -> String {
    let mut result = String::new();
    for c in s.chars() {
        match c {
            // 编码行为与 JavaScript 的 `encodeURIComponent` 及 AList/OpenList 签名逻辑一致。
            '!' | '\'' | '(' | ')' | '*' => result.push(c),
            ' ' => result.push_str("%20"),
            _ => {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '~' {
                    result.push(c);
                } else {
                    for b in c.to_string().as_bytes() {
                        result.push_str(&format!("%{:02X}", b));
                    }
                }
            }
        }
    }
    result
}

pub fn calc_sign(body: &str, ts: &str, rand_str: &str) -> String {
    let encoded = encode_uri_component(body);
    let mut chars: Vec<char> = encoded.chars().collect();
    chars.sort();
    let sorted: String = chars.into_iter().collect();

    let body_base64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &sorted);

    let hash1 = md5_hash(&body_base64);
    let hash2 = md5_hash(&format!("{}:{}", ts, rand_str));

    let combined = format!("{}{}", hash1, hash2);
    md5_hash(&combined).to_uppercase()
}

pub fn calc_file_sha256(path: impl AsRef<std::path::Path>) -> Result<String, std::io::Error> {
    use std::fs::File;
    use std::io::Read;

    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];

    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(hex::encode(hasher.finalize()))
}
