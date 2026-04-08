# cloud139 代码优化实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 全面提升 cloud139 代码健壮性与可维护性，消除 panic 点，统一错误处理，消除重复代码，提升测试覆盖率至 ~80%

**Architecture:** 自底向上三阶段：(1) 错误处理基础设施，(2) 消除重复代码，(3) 补充测试。thiserror 用于库层错误类型，anyhow 用于命令层错误传播。

**Tech Stack:** Rust 2024, anyhow 1.x, thiserror 2.x, httpmock 0.8, reqwest 0.12

---

## Phase 1: 错误处理基础设施

### Task 1: 添加 anyhow 依赖

**Files:**
- `Cargo.toml`

- [ ] 在 `Cargo.toml` 的 `[dependencies]` 段中添加 `anyhow = "1"`：

```toml
[dependencies]
anyhow = "1"
# ... existing deps
```

- [ ] 运行验证：

```bash
cargo check
# 期望输出: Compiling cloud139 ... Finished
```

- [ ] 提交：

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore: add anyhow dependency"
```

---

### Task 2: 修复 utils/crypto.rs 错误类型

**Files:**
- `src/utils/crypto.rs`

- [ ] 将文件顶部的导入从 `use std::error::Error;` 改为：

```rust
use anyhow::{anyhow, Result};
```

- [ ] 将所有 4 个函数签名从 `Box<dyn Error>` 改为 `anyhow::Result<T>`：

```rust
pub fn aes_cbc_encrypt(key: &[u8], iv: &[u8], data: &[u8]) -> Result<Vec<u8>> { ... }
pub fn aes_cbc_decrypt(key: &[u8], iv: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>> { ... }
pub fn aes_ecb_decrypt(key: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>> { ... }
pub fn pkcs7_unpad(data: &[u8]) -> Result<Vec<u8>> { ... }
```

- [ ] 将字符串错误字面量改为带上下文的 `anyhow!()` 宏，例如：

```rust
// 旧写法
return Err("ciphertext is not a multiple of the block size".into());

// 新写法
return Err(anyhow!(
    "ciphertext length {} is not a multiple of block size 16",
    ciphertext.len()
));
```

```rust
// pkcs7_unpad 中的空数据错误
return Err(anyhow!("pkcs7_unpad: input data is empty"));

// 无效 padding 错误
return Err(anyhow!(
    "pkcs7_unpad: invalid padding byte {} at position {}",
    pad_byte, i
));
```

- [ ] 运行验证：

```bash
cargo test --test crypto_test
# 期望输出: test result: ok. N passed; 0 failed
```

- [ ] 提交：

```bash
git add src/utils/crypto.rs
git commit -m "refactor(crypto): use anyhow::Result instead of Box<dyn Error>"
```

---

### Task 3: 提取常量并修复 client/mod.rs 请求头

**Files:**
- `src/client/mod.rs`

- [ ] 在文件顶部（`use` 语句之后，结构体定义之前）添加常量：

```rust
pub const MCLOUD_VERSION: &str = "7.14.0";
pub const MCLOUD_CLIENT: &str = "10701";
pub const MCLOUD_CHANNEL: &str = "1000101";
pub const MCLOUD_CHANNEL_SRC: &str = "10000034";
pub const DEVICE_INFO: &str = "||9|7.14.0|chrome|120.0.0.0|||windows 10||zh-CN|||";
pub const CLIENT_INFO: &str = "||9|7.14.0|chrome|120.0.0.0|||windows 10||zh-CN|||dW5kZWZpbmVk||";
```

- [ ] 在 `ClientError` 枚举中添加 `InvalidHeader` 变体：

```rust
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    // ... existing variants ...
    #[error("无效的请求头: {0}")]
    InvalidHeader(String),
}
```

- [ ] 将 `build_headers()` 签名改为返回 `Result`，并区分静态/动态头：

```rust
fn build_headers(
    &self,
    ts: &str,
    rand_str: &str,
    sign: &str,
) -> Result<reqwest::header::HeaderMap, ClientError> {
    use reqwest::header::{HeaderMap, HeaderValue};
    let mut headers = HeaderMap::new();

    // 静态值：使用 from_static()，零开销，无 unwrap
    headers.insert("mcloud-client", HeaderValue::from_static(MCLOUD_CLIENT));
    headers.insert("mcloud-channel", HeaderValue::from_static(MCLOUD_CHANNEL));
    headers.insert("mcloud-channel-src", HeaderValue::from_static(MCLOUD_CHANNEL_SRC));
    headers.insert("mcloud-version", HeaderValue::from_static(MCLOUD_VERSION));
    headers.insert("deviceInfo", HeaderValue::from_static(DEVICE_INFO));
    headers.insert("clientInfo", HeaderValue::from_static(CLIENT_INFO));

    // 动态值：parse() + map_err
    headers.insert(
        "Authorization",
        format!("Bearer {}", self.token)
            .parse()
            .map_err(|e| ClientError::InvalidHeader(format!("{}", e)))?,
    );
    headers.insert(
        "mcloud-sign",
        format!("{},{},{}", ts, rand_str, sign)
            .parse()
            .map_err(|e| ClientError::InvalidHeader(format!("{}", e)))?,
    );
    headers.insert(
        "x-SvcType",
        self.svc_type
            .parse()
            .map_err(|e| ClientError::InvalidHeader(format!("{}", e)))?,
    );

    Ok(headers)
}
```

- [ ] 修复 `Client::new()` 中的 `.unwrap()`：

```rust
// 旧写法
.build().unwrap()

// 新写法
.build().expect("Failed to build HTTP client")
```

- [ ] 更新所有调用 `build_headers()` 的地方，加上 `?` 传播错误：

```rust
let headers = self.build_headers(ts, rand_str, sign)?;
```

- [ ] 运行验证：

```bash
cargo check
# 期望输出: Finished
```

- [ ] 提交：

```bash
git add src/client/mod.rs
git commit -m "refactor(client): extract constants, fix header construction, add InvalidHeader variant"
```

---

### Task 4: 修复 client/api.rs 请求头

**Files:**
- `src/client/api.rs`

- [ ] 在文件顶部添加常量导入（Task 3 中已将常量设为 `pub`）：

```rust
use crate::client::{
    CLIENT_INFO, DEVICE_INFO, MCLOUD_CHANNEL, MCLOUD_CHANNEL_SRC, MCLOUD_CLIENT, MCLOUD_VERSION,
};
```

- [ ] 在 `get_personal_cloud_host_with_client()` 中，将所有 `.parse().unwrap()` 替换：

```rust
// 静态值
headers.insert("mcloud-client", HeaderValue::from_static(MCLOUD_CLIENT));
headers.insert("mcloud-channel", HeaderValue::from_static(MCLOUD_CHANNEL));
headers.insert("mcloud-channel-src", HeaderValue::from_static(MCLOUD_CHANNEL_SRC));
headers.insert("mcloud-version", HeaderValue::from_static(MCLOUD_VERSION));
headers.insert("deviceInfo", HeaderValue::from_static(DEVICE_INFO));
headers.insert("clientInfo", HeaderValue::from_static(CLIENT_INFO));

// 动态值
headers.insert(
    "Authorization",
    format!("Bearer {}", token)
        .parse()
        .map_err(|e| ClientError::InvalidHeader(format!("{}", e)))?,
);
```

- [ ] 在 `personal_api_request_with_client()` 中做同样的替换。

- [ ] 运行验证：

```bash
cargo check
# 期望输出: Finished
```

- [ ] 提交：

```bash
git add src/client/api.rs
git commit -m "refactor(api): use constants and eliminate .parse().unwrap() in header construction"
```

---

### Task 5: 修复 commands/*.rs 返回类型

**Files:**
- `src/commands/upload.rs`
- `src/commands/download.rs`
- `src/commands/delete.rs`
- `src/commands/mv.rs`
- `src/commands/cp.rs`
- `src/commands/rename.rs`
- `src/commands/list.rs`
- `src/commands/login.rs`
- `src/commands/mkdir.rs`

- [ ] 对每个文件执行以下修改：

1. 在文件顶部添加导入：

```rust
use anyhow::Context;
```

2. 将 `execute()` 返回类型从 `Result<(), ClientError>` 改为 `anyhow::Result<()>`：

```rust
// 旧写法
pub async fn execute(args: Args) -> Result<(), ClientError> {

// 新写法
pub async fn execute(args: Args) -> anyhow::Result<()> {
```

3. 将配置加载错误改为带上下文的传播：

```rust
// 旧写法
let config = Config::load().map_err(ClientError::Config)?;

// 新写法
let config = Config::load().context("加载配置失败")?;
```

4. 将其余 `.unwrap()` 调用替换为 `.ok_or_else()` 或 `.context()`：

```rust
// 旧写法
let value = some_option.unwrap();

// 新写法
let value = some_option.context("获取值失败")?;
// 或
let value = some_option.ok_or_else(|| anyhow::anyhow!("描述具体缺失的值"))?;
```

- [ ] 运行验证：

```bash
cargo check
# 期望输出: Finished
```

- [ ] 提交：

```bash
git add src/commands/
git commit -m "refactor(commands): change execute() return type to anyhow::Result, add context to errors"
```

---

### Task 6: 消除 download.rs 中的重复路径解析逻辑

**Files:**
- `src/commands/download.rs`

- [ ] 确认 `resolve_local_path()` 函数已在文件中定义（约第 16-49 行），签名类似：

```rust
fn resolve_local_path(remote_path: &str, local_path: &Option<String>) -> PathBuf {
    // ... 已有实现
}
```

- [ ] 在 `execute()` 函数中（约第 56-93 行），找到重复的路径解析块并替换为单次函数调用：

```rust
// 删除这段重复代码（约 30-40 行）：
// let local_path = if let Some(ref lp) = args.local_path {
//     if lp.ends_with('/') || lp.ends_with('\\') { ... }
//     ...
// } else { ... };

// 替换为：
let local_path = resolve_local_path(&remote_path, &args.local_path);
```

- [ ] 运行验证：

```bash
cargo test --test download_test
# 期望输出: test result: ok. N passed; 0 failed
```

- [ ] 提交：

```bash
git add src/commands/download.rs
git commit -m "refactor(download): eliminate duplicated path resolution, call resolve_local_path()"
```

---

### Task 7: 修复 main.rs

**Files:**
- `src/main.rs`

- [ ] 在文件顶部添加导入：

```rust
use anyhow::Context;
```

- [ ] 将 `main()` 签名从 `Box<dyn std::error::Error>` 改为 `anyhow::Result`：

```rust
// 旧写法
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

// 新写法
#[tokio::main]
async fn main() -> anyhow::Result<()> {
```

- [ ] 运行完整构建验证：

```bash
cargo build
# 期望输出: Compiling cloud139 ... Finished dev [unoptimized + debuginfo]
```

- [ ] 提交：

```bash
git add src/main.rs
git commit -m "refactor(main): use anyhow::Result in main()"
```

---

## Phase 2: 补充测试

### Task 8: crypto.rs 边界条件测试

**Files:**
- `tests/crypto_test.rs` （或新建 `tests/utils_crypto_edge_test.rs`）

- [ ] 添加以下测试用例：

```rust
#[cfg(test)]
mod crypto_edge_tests {
    use cloud139::utils::crypto::*;

    #[test]
    fn test_aes_ecb_decrypt_non_multiple_block_size() {
        let key = b"0123456789abcdef"; // 16 bytes
        let bad_ciphertext = vec![0u8; 15]; // 非 16 的倍数
        let result = aes_ecb_decrypt(key, &bad_ciphertext);
        assert!(result.is_err(), "应当返回 Err，因为密文长度不是块大小的倍数");
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("15"), "错误信息应包含实际长度 15");
    }

    #[test]
    fn test_pkcs7_unpad_empty() {
        let result = pkcs7_unpad(&[]);
        assert!(result.is_err(), "空输入应返回 Err");
        assert!(result.unwrap_err().to_string().contains("empty"));
    }

    #[test]
    fn test_pkcs7_unpad_invalid_padding() {
        // 最后一个字节声明 padding 为 5，但前面的字节不全是 5
        let mut data = vec![0u8; 16];
        data[15] = 5;
        data[14] = 3; // 错误的 padding 字节
        let result = pkcs7_unpad(&data);
        assert!(result.is_err(), "无效 padding 应返回 Err");
    }

    #[test]
    fn test_pkcs7_unpad_valid() {
        // "hello" + 11 字节的 padding (0x0b)
        let mut data = b"hello".to_vec();
        let pad_len = 16 - data.len();
        data.extend(vec![pad_len as u8; pad_len]);
        let result = pkcs7_unpad(&data);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), b"hello");
    }

    #[test]
    fn test_aes_cbc_roundtrip() {
        let key = b"0123456789abcdef";
        let iv = b"abcdef0123456789";
        let plaintext = b"Hello, cloud139!";
        let encrypted = aes_cbc_encrypt(key, iv, plaintext).expect("加密失败");
        let decrypted = aes_cbc_decrypt(key, iv, &encrypted).expect("解密失败");
        assert_eq!(decrypted, plaintext, "解密后应还原原始数据");
    }
}
```

- [ ] 运行验证：

```bash
cargo test --test crypto_test
# 期望输出: test result: ok. 5 passed; 0 failed
```

- [ ] 提交：

```bash
git add tests/crypto_test.rs
git commit -m "test(crypto): add edge case tests for aes_ecb_decrypt, pkcs7_unpad, and cbc roundtrip"
```

---

### Task 9: client/api.rs httpmock 测试

**Files:**
- `tests/api_mock_test.rs`

- [ ] 添加以下测试用例：

```rust
#[cfg(test)]
mod api_mock_tests {
    use cloud139::client::Client;
    use cloud139::config::Config;
    use httpmock::prelude::*;
    use serde_json::json;

    fn make_test_config(server: &MockServer) -> Config {
        Config {
            token: "test-token".to_string(),
            personal_cloud_host: None,
            // 其他字段使用默认值
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn test_get_personal_cloud_host_success() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST).path("/user/route/qryRoutePolicy");
            then.status(200).json_body(json!({
                "data": { "userSiteInfo": [{ "siteIp": "cloud.example.com" }] }
            }));
        });

        let mut config = make_test_config(&server);
        let client = Client::new_with_base_url(&config, server.base_url());
        let host = client.get_personal_cloud_host(&mut config).await;

        mock.assert();
        assert!(host.is_ok());
        assert_eq!(host.unwrap(), "cloud.example.com");
    }

    #[tokio::test]
    async fn test_get_personal_cloud_host_cached() {
        let server = MockServer::start();
        // 不注册任何 mock，若发出 HTTP 请求则测试失败

        let mut config = Config {
            personal_cloud_host: Some("cached-host.example.com".to_string()),
            ..Default::default()
        };
        let client = Client::new_with_base_url(&config, server.base_url());
        let host = client.get_personal_cloud_host(&mut config).await;

        assert!(host.is_ok());
        assert_eq!(host.unwrap(), "cached-host.example.com");
    }

    #[tokio::test]
    async fn test_personal_api_request_success() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/api/test-endpoint");
            then.status(200).json_body(json!({ "result": "ok" }));
        });

        let config = make_test_config(&server);
        let client = Client::new_with_base_url(&config, server.base_url());
        let resp: serde_json::Value = client
            .personal_api_request("/api/test-endpoint", json!({}))
            .await
            .expect("请求应成功");

        assert_eq!(resp["result"], "ok");
    }

    #[tokio::test]
    async fn test_check_file_exists_found() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST);
            then.status(200).json_body(json!({
                "data": { "fileListAO": { "fileList": [{ "name": "target.txt" }] } }
            }));
        });

        let config = make_test_config(&server);
        let client = Client::new_with_base_url(&config, server.base_url());
        let exists = client
            .check_file_exists("/remote/path/target.txt")
            .await
            .expect("检查应成功");

        assert!(exists, "文件存在时应返回 true");
    }

    #[tokio::test]
    async fn test_check_file_exists_not_found() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST);
            then.status(200).json_body(json!({
                "data": { "fileListAO": { "fileList": [] } }
            }));
        });

        let config = make_test_config(&server);
        let client = Client::new_with_base_url(&config, server.base_url());
        let exists = client
            .check_file_exists("/remote/path/missing.txt")
            .await
            .expect("检查应成功");

        assert!(!exists, "文件不存在时应返回 false");
    }
}
```

- [ ] 运行验证：

```bash
cargo test --test api_mock_test
# 期望输出: test result: ok. 5 passed; 0 failed
```

- [ ] 提交：

```bash
git add tests/api_mock_test.rs
git commit -m "test(api): add httpmock tests for get_personal_cloud_host and check_file_exists"
```

---

### Task 10: commands/download.rs 路径解析测试

**Files:**
- `tests/download_mock_test.rs`

- [ ] 添加以下测试用例（扩展现有文件或新建）：

```rust
#[cfg(test)]
mod download_path_tests {
    // resolve_local_path 需要设为 pub(crate) 或通过集成测试访问
    // 若为私有函数，可在 src/commands/download.rs 中添加 #[cfg(test)] 模块进行单元测试

    #[test]
    fn test_resolve_local_path_with_directory() {
        // local_path 以 '/' 结尾，表示目录，应追加远程文件名
        let result = resolve_local_path("/remote/path/file.txt", &Some("/local/dir/".to_string()));
        assert_eq!(result.file_name().unwrap().to_str().unwrap(), "file.txt");
        assert!(result.to_str().unwrap().starts_with("/local/dir/"));
    }

    #[test]
    fn test_resolve_local_path_no_local_path() {
        // 未指定本地路径，应使用远程文件名
        let result = resolve_local_path("/remote/path/myfile.zip", &None);
        assert_eq!(result.to_str().unwrap(), "myfile.zip");
    }

    #[test]
    fn test_resolve_local_path_explicit_file() {
        // 指定了完整本地文件路径，直接使用
        let result = resolve_local_path(
            "/remote/path/original.txt",
            &Some("/local/renamed.txt".to_string()),
        );
        assert_eq!(result.to_str().unwrap(), "/local/renamed.txt");
    }

    #[test]
    fn test_resolve_local_path_empty_remote() {
        // 远程路径为空，应回退到 "download"
        let result = resolve_local_path("", &None);
        assert_eq!(result.to_str().unwrap(), "download");
    }
}
```

- [ ] 运行验证：

```bash
cargo test --test download_mock_test
# 期望输出: test result: ok. 4 passed; 0 failed
```

- [ ] 提交：

```bash
git add tests/download_mock_test.rs
git commit -m "test(download): add unit tests for resolve_local_path edge cases"
```

---

### Task 11: commands/upload.rs 分片大小测试

**Files:**
- `tests/upload_mock_test.rs`

- [ ] 添加以下测试用例（扩展现有文件）：

```rust
#[cfg(test)]
mod upload_part_size_tests {
    // get_part_size 需要设为 pub(crate) 以便测试访问
    use cloud139::commands::upload::get_part_size;

    const MB: u64 = 1024 * 1024;
    const GB: u64 = 1024 * MB;

    #[test]
    fn test_get_part_size_default_small_file() {
        // 文件小于 30GB，默认分片为 100MB
        let size = get_part_size(10 * GB, 0);
        assert_eq!(size, 100 * MB, "小文件默认分片应为 100MB");
    }

    #[test]
    fn test_get_part_size_default_large_file() {
        // 文件大于 30GB，默认分片为 512MB
        let size = get_part_size(31 * GB, 0);
        assert_eq!(size, 512 * MB, "大文件默认分片应为 512MB");
    }

    #[test]
    fn test_get_part_size_custom() {
        // 指定了自定义分片大小，直接返回
        let custom = 256 * MB;
        let size = get_part_size(100 * GB, custom);
        assert_eq!(size, custom, "自定义分片大小应原样返回");
    }
}
```

- [ ] 运行验证：

```bash
cargo test --test upload_mock_test
# 期望输出: test result: ok. 3 passed; 0 failed
```

- [ ] 提交：

```bash
git add tests/upload_mock_test.rs
git commit -m "test(upload): add unit tests for get_part_size logic"
```

---

### Task 12: config 测试

**Files:**
- `tests/config_test.rs`

- [ ] 添加以下测试用例（扩展现有文件）：

```rust
#[cfg(test)]
mod config_extra_tests {
    use cloud139::config::{Config, StorageType};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn secs_from_now(offset_secs: i64) -> u64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        (now as i64 + offset_secs) as u64
    }

    #[test]
    fn test_config_storage_type_family() {
        let config = Config {
            storage_type: "family".to_string(),
            ..Default::default()
        };
        assert_eq!(config.get_storage_type(), StorageType::Family);
    }

    #[test]
    fn test_config_storage_type_group() {
        let config = Config {
            storage_type: "group".to_string(),
            ..Default::default()
        };
        assert_eq!(config.get_storage_type(), StorageType::Group);
    }

    #[test]
    fn test_config_storage_type_default() {
        let config = Config {
            storage_type: "unknown_value".to_string(),
            ..Default::default()
        };
        assert_eq!(config.get_storage_type(), StorageType::PersonalNew);
    }

    #[test]
    fn test_config_is_token_expired_no_expire_time() {
        let config = Config {
            token_expire_time: None,
            ..Default::default()
        };
        assert!(config.is_token_expired(), "无过期时间时应视为已过期");
    }

    #[test]
    fn test_config_is_token_expired_future() {
        let config = Config {
            token_expire_time: Some(secs_from_now(3600)), // 1 小时后过期
            ..Default::default()
        };
        assert!(!config.is_token_expired(), "未来过期时间应视为未过期");
    }
}
```

- [ ] 运行验证：

```bash
cargo test --test config_test
# 期望输出: test result: ok. 5 passed; 0 failed
```

- [ ] 提交：

```bash
git add tests/config_test.rs
git commit -m "test(config): add tests for StorageType parsing and token expiry logic"
```

---

### Task 13: 最终验证

**Files:** 全部

- [ ] 运行全量测试：

```bash
cargo test
# 期望输出: test result: ok. N passed; 0 failed; 0 ignored
```

- [ ] 运行 clippy 检查：

```bash
cargo clippy -- -D warnings
# 期望输出: 无 warning，Finished
```

- [ ] 若 clippy 报告问题，修复后提交：

```bash
git add -p
git commit -m "fix(clippy): address remaining lint warnings"
```

- [ ] 最终提交（若有剩余修改）：

```bash
git add .
git commit -m "chore: final cleanup after optimization pass"
```

---

## 变更摘要

| 阶段 | 任务 | 影响范围 |
|------|------|----------|
| Phase 1 | Task 1-2 | 依赖 + crypto 错误类型 |
| Phase 1 | Task 3-4 | client 层常量提取 + 请求头安全化 |
| Phase 1 | Task 5-7 | commands 层统一 anyhow，消除 panic |
| Phase 2 | Task 8-12 | 新增 ~20 个测试用例，覆盖边界条件 |
| Phase 2 | Task 13 | 全量验证 + clippy 清零 |
