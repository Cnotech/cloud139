use aes::cipher::generic_array::GenericArray;
use anyhow::{Result, anyhow};
use digest::Digest;

pub fn sha1_hash(data: &str) -> String {
    let mut hasher = sha1::Sha1::new();
    hasher.update(data.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn md5_hash(data: &str) -> String {
    let mut hasher = md5::Md5::new();
    hasher.update(data.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn aes_cbc_encrypt(plaintext: &[u8], key: &[u8], iv: &[u8]) -> Result<Vec<u8>> {
    use aes::Aes128;
    use cbc::{
        Encryptor,
        cipher::{BlockEncryptMut, KeyIvInit},
    };

    type Aes128Cbc = Encryptor<Aes128>;

    let mut cipher = Aes128Cbc::new(key.into(), iv.into());
    let mut data = plaintext.to_vec();
    let len = data.len();
    let pad_len = if len.is_multiple_of(16) {
        16
    } else {
        16 - (len % 16)
    };
    data.extend(vec![pad_len as u8; pad_len]);

    let mut blocks: Vec<GenericArray<u8, typenum::U16>> = Vec::new();
    for chunk in data.chunks(16) {
        let mut block = GenericArray::from([0u8; 16]);
        block.copy_from_slice(chunk);
        blocks.push(block);
    }

    cipher.encrypt_blocks_mut(&mut blocks);

    let mut ciphertext = Vec::with_capacity(data.len());
    for block in blocks {
        ciphertext.extend_from_slice(&block);
    }

    Ok(ciphertext)
}

pub fn aes_cbc_decrypt(ciphertext: &[u8], key: &[u8], iv: &[u8]) -> Result<Vec<u8>> {
    use aes::Aes128;
    use cbc::{
        Decryptor,
        cipher::{BlockDecryptMut, KeyIvInit},
    };

    type Aes128Cbc = Decryptor<Aes128>;

    let mut cipher = Aes128Cbc::new(key.into(), iv.into());

    let mut blocks: Vec<GenericArray<u8, typenum::U16>> = Vec::new();
    for chunk in ciphertext.chunks(16) {
        let mut block = GenericArray::from([0u8; 16]);
        block.copy_from_slice(chunk);
        blocks.push(block);
    }

    cipher.decrypt_blocks_mut(&mut blocks);

    let mut plaintext = Vec::with_capacity(ciphertext.len());
    for block in blocks {
        plaintext.extend_from_slice(&block);
    }

    let padding = plaintext[plaintext.len() - 1] as usize;
    if padding > 0 && padding <= 16 {
        plaintext.truncate(plaintext.len() - padding);
    }

    Ok(plaintext)
}

pub fn aes_ecb_decrypt(ciphertext: &[u8], key: &[u8]) -> Result<Vec<u8>> {
    use aes::cipher::{BlockDecryptMut, KeyInit};

    let block_size = 16;

    if ciphertext.is_empty() {
        return Err(anyhow!("ciphertext is empty"));
    }

    if ciphertext.len() < block_size || !ciphertext.len().is_multiple_of(block_size) {
        return Err(anyhow!(
            "ciphertext length {} is not a multiple of block size 16",
            ciphertext.len()
        ));
    }

    let mut cipher = aes::Aes128::new(key.into());

    let mut result = ciphertext.to_vec();
    let blocks = result.chunks_mut(block_size);

    for block in blocks {
        let mut arr = *GenericArray::<u8, typenum::U16>::from_slice(block);
        cipher.decrypt_block_mut(&mut arr);
        block.copy_from_slice(&arr);
    }

    let padding = result[result.len() - 1] as usize;
    if padding > 0 && padding <= 16 {
        result.truncate(result.len() - padding);
    }

    Ok(result)
}

pub fn pkcs7_pad(data: &[u8], block_size: usize) -> Vec<u8> {
    let padding = block_size - (data.len() % block_size);
    let mut result = data.to_vec();
    result.extend(vec![padding as u8; padding]);
    result
}

pub fn pkcs7_unpad(data: &[u8]) -> Result<Vec<u8>> {
    if data.is_empty() {
        return Err(anyhow!("pkcs7_unpad: input data is empty"));
    }
    let padding = data[data.len() - 1] as usize;
    if padding > data.len() || padding == 0 {
        return Err(anyhow!("pkcs7_unpad: invalid padding"));
    }
    for i in 0..padding {
        if data[data.len() - 1 - i] != padding as u8 {
            return Err(anyhow!(
                "pkcs7_unpad: invalid padding byte {} at position {}",
                data[data.len() - 1 - i],
                data.len() - 1 - i
            ));
        }
    }
    Ok(data[..data.len() - padding].to_vec())
}

pub fn encode_uri_component(s: &str) -> String {
    let mut result = String::new();
    for c in s.chars() {
        match c {
            '!' => result.push_str("%21"),
            '\'' => result.push_str("%27"),
            '(' => result.push_str("%28"),
            ')' => result.push_str("%29"),
            '*' => result.push_str("%2A"),
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

pub fn calc_file_hash(path: &str) -> Result<String, std::io::Error> {
    use std::fs::File;
    use std::io::Read;

    let mut file = File::open(path)?;
    let mut hasher = sha1::Sha1::new();
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

pub fn calc_file_sha256(path: &str) -> Result<String, std::io::Error> {
    use std::fs::File;
    use std::io::Read;

    let mut file = File::open(path)?;
    let mut hasher = sha2::Sha256::new();
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

pub fn generate_random_string(len: usize) -> String {
    use rand::Rng;
    const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut rng = rand::thread_rng();
    (0..len)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}
