use rand::distributions::{Alphanumeric, DistString};
use sha2::{Digest, Sha256};

pub fn generate_rand_str(len: usize) -> String {
    Alphanumeric.sample_string(&mut rand::thread_rng(), len)
}

pub(crate) fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub fn md5_hash(data: &str) -> String {
    let mut hasher = md5::Md5::new();
    hasher.update(data.as_bytes());
    hex::encode(hasher.finalize())
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
