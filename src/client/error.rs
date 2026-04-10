use thiserror::Error;

#[derive(Error, Debug)]
pub enum ClientError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("API error: {0}")]
    Api(String),
    #[error("Not logged in")]
    NotLoggedIn,
    #[error("Token expired")]
    TokenExpired,
    #[error("Config error: {0}")]
    Config(#[from] crate::config::ConfigError),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Other error: {0}")]
    Other(String),
    #[error("force flag required")]
    ForceRequired,
    #[error("confirmation flag required (--yes)")]
    ConfirmationRequired,
    #[error("invalid source path")]
    InvalidSourcePath,
    #[error("file not found")]
    FileNotFound,
    #[error("cannot operate on root directory")]
    CannotOperateOnRoot,
    #[error("no source files to process")]
    NoSourceFiles,
    #[error("family storage does not support batch move")]
    UnsupportedFamilyBatchMove,
    #[error("group storage does not support batch move")]
    UnsupportedGroupBatchMove,
    #[error("family storage does not support renaming folders")]
    UnsupportedFamilyRenameFolder,
    #[error("downloading directories is not supported")]
    UnsupportedDownloadDirectory,
    #[error("invalid file path")]
    InvalidFilePath,
    #[error("operation cancelled")]
    OperationCancelled,
    #[error("invalid request header: {0}")]
    InvalidHeader(String),
}
