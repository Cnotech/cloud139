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
use crate::client::{ClientError, api::HttpClientWrapper};
use crate::domain::EntryKind;
use crate::commands::list;
use crate::models::FileInfo;
use crate::{error, info, success, warn};
```

## 命名约定

| 类型 | 命名规范 | 示例 |
|------|----------|------|
| 模块/文件名 | snake_case | `client_api.rs` |
| Struct/Enum | PascalCase | `ClientError`, `EntryKind` |
| 函数/方法 | snake_case | `parse_path`, `execute` |
| 变量 | snake_case | `storage_type`, `file_info` |
| 常量 | SCREAMING_SNAKE_CASE | `KEY_HEX_1` |
| Trait | PascalCase | `ApiTrait` |

### 条件操作

- 条件操作先写动作和对象，再用 `_if_<具体条件>` 标明执行条件。避免用 `maybe_` 或 `_if_needed` 代替具体条件。
- 将调用方需要知道的副作用写进名称。`refresh_token_if_due` 返回更新后的配置，`refresh_saved_token` 还会把结果写回配置文件。
- 只返回布尔值的判断用 `is_` 或 `has_` 等谓词，如 `is_token_refresh_due`。在文档中说明具体触发条件。

### 破坏性变更

删除废弃的 API 和命令行参数时，提升 SemVer 主版本，并在升级说明中列出受影响的用法和迁移方式。

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
pub async fn execute(path: &std::path::Path) -> Result<(), ClientError> {
    let config = crate::config::Config::load_from_path(path)?;
    config.require_personal()?;
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

测试文件的组织方式见 [AGENTS.md 的测试文件编写规范](../AGENTS.md#测试文件编写规范)。

```rust
#[test]
fn test_parse_path_root() {
    let result = mkdir::parse_path("/test");
    assert!(result.is_ok());
    let (parent, name) = result.unwrap();
    assert_eq!(parent, "/");
    assert_eq!(name, "test");
}
```
