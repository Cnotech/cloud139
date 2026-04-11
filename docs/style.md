# 代码风格指南

## 导入规范

```rust
// 标准库导入
use std::io::Error;

// 外部 crates
use clap::Parser;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::fs;

// 项目内部导入（使用 crate:: 前缀）
use crate::client::{Client, ClientError, StorageType};
use crate::commands::list;
use crate::models::FileInfo;
use crate::{error, info, success, warn};
```

## 命名约定

| 类型 | 命名规范 | 示例 |
|------|----------|------|
| 模块/文件名 | snake_case | `client_api.rs` |
| Struct/Enum | PascalCase | `ClientError`, `StorageType` |
| 函数/方法 | snake_case | `parse_path`, `execute` |
| 变量 | snake_case | `storage_type`, `file_info` |
| 常量 | SCREAMING_SNAKE_CASE | `KEY_HEX_1` |
| Trait | PascalCase | `ApiTrait` |

## 错误处理

```rust
// 使用 thiserror 定义错误类型
#[derive(Error, Debug)]
pub enum ClientError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("API error: {0}")]
    Api(String),
    #[error("Not logged in")]
    NotLoggedIn,
    #[error("Config error: {0}")]
    Config(#[from] crate::config::ConfigError),
}

// 在函数中返回 Result，使用 ? 操作符传播错误
pub async fn execute(args: Args) -> Result<(), ClientError> {
    let config = crate::config::Config::load().map_err(ClientError::Config)?;
    Ok(())
}

// 打印错误并退出
if let Err(e) = result {
    error!("{}", e);
    std::process::exit(1);
}
```

## 日志与输出

```rust
use crate::{error, info, success, warn};

info!("正在登录...");
success!("登录成功");
warn!("文件已存在");
error!("登录失败: {}", e);
```

## CLI 定义

```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "cloud139")]
#[command(about = "139 Yun CLI - 移动云盘命令行工具", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    #[arg(short, long, default_value = "info")]
    verbose: String,
}

#[derive(Subcommand)]
enum Commands {
    /// 登录账号
    Login(login::LoginArgs),
    /// 列出文件
    Ls(list::ListArgs),
    // ...其他命令
}
```

## 异步编程

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let content = tokio::fs::read(&path).await?;
    Ok(())
}
```

## 测试规范

```rust
// 测试文件放在 tests/ 目录
#[test]
fn test_parse_path_root() {
    let result = mkdir::parse_path("/test");
    assert!(result.is_ok());
    let (parent, name) = result.unwrap();
    assert_eq!(parent, "/");
    assert_eq!(name, "test");
}
```
