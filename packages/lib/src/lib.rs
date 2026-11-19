//! 中国移动个人云盘的 Rust 客户端，支持 `personal_new` 存储类型。
//!
//! ## 快速开始
//!
//! 使用 [`Config::load_from_path`](config::Config::load_from_path) 读取已有的 TOML 配置，
//! 或用 [`login`](services::login::login) 验证 Token 并取得配置。异步接口在 Tokio 运行时中调用。
//! 此示例使用 Tokio 的 `macros` 和 `rt` 特性。
//!
//! ```no_run
//! use cloud139::config::Config;
//! use cloud139::services::list::{ListOptions, list};
//! use std::path::Path;
//!
//! #[tokio::main(flavor = "current_thread")]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = Config::load_from_path(Path::new("cloud139.toml"))?;
//!     let options = ListOptions { path: "/", page: 1, page_size: 20 };
//!     let result = list(&config, options).await?;
//!     for item in result.items {
//!         println!("{}", item.name);
//!     }
//!     Ok(())
//! }
//! ```
//!
//! ## 常用接口
//!
//! - [`services`] 提供按路径列出、上传、下载和管理云端文件的接口。云端路径以 `/` 分隔，根目录为 `/`。
//! - [`services::sync`] 提供本地与云端之间的单向同步，可先查看计划再执行。
//! - [`client::auth::refresh`] 提供显式续期接口。续期返回新配置，由调用方决定何时用
//!   [`Config::save_to`](config::Config::save_to) 写回。
//! - [`progress`] 定义调用方可实现的传输进度接口。

#![warn(unreachable_pub)]

pub mod client;
pub mod config;
pub mod error;
pub mod models;
pub mod progress;
mod protocol;
pub mod services;
pub mod utils;
