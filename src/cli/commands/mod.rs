// src/cli/commands/mod.rs
pub mod cp;
pub mod delete;
pub mod download;
pub mod login;
pub mod mkdir;
pub mod mv;
pub mod rename;
pub mod sync;
pub mod upload;

pub use cp::CpArgs;
pub use delete::DeleteArgs;
pub use download::DownloadArgs;
pub use login::LoginArgs;
pub use mkdir::MkdirArgs;
pub use mv::MvArgs;
pub use rename::RenameArgs;
pub use sync::SyncArgs;
pub use upload::UploadArgs;
