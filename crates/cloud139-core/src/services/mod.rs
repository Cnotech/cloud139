// src/application/services/mod.rs
pub mod copy_service;
pub mod delete_service;
pub mod download_service;
pub mod list_service;
pub mod login_service;
pub mod mkdir_service;
pub mod move_service;
pub mod progress;
pub mod rename_service;
pub mod sync_executor;
mod sync_local;
mod sync_remote;
pub mod sync_report;
pub mod sync_service;
pub(crate) mod sync_verify;
pub mod upload;
pub mod upload_service;

pub use copy_service::cp;
pub use delete_service::delete;
pub use download_service::download;
pub use list_service::{ListResult, list};
pub use mkdir_service::mkdir;
pub use move_service::mv;
pub use rename_service::rename;
pub use upload_service::upload;

pub mod directory_service;
