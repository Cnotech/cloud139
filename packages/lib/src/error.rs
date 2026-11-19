//! 库的错误类型与通用错误构造。

use thiserror::Error;

/// 认证、云端请求和文件操作可能返回的错误。
#[derive(Error, Debug)]
pub enum ClientError {
    #[error("HTTP 错误: {0}")]
    Http(#[from] reqwest::Error),
    #[error("API 错误: {0}")]
    Api(String),
    #[error("未登录")]
    NotLoggedIn,
    #[error("Token 已过期")]
    TokenExpired,
    #[error("配置错误: {0}")]
    Config(#[from] ConfigError),
    #[error("JSON 错误: {0}")]
    Json(#[from] serde_json::Error),
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),
    #[error("其他错误: {0}")]
    Other(String),
    #[error("请使用 --force 参数确认继续")]
    ForceRequired,
    #[error("请使用 --yes 参数确认删除")]
    ConfirmationRequired,
    #[error("无效的源文件路径")]
    InvalidSourcePath,
    #[error("文件不存在")]
    FileNotFound,
    #[error("不能操作根目录")]
    CannotOperateOnRoot,
    #[error("没有有效的源文件需要处理")]
    NoSourceFiles,
    #[error("未实现：当前仅支持个人云")]
    UnsupportedStorage,
    #[error("下载目录需要显式指定 -r / --recursive")]
    UnsupportedDownloadDirectory,
    #[error("无效的文件路径")]
    InvalidFilePath,
    #[error("操作被取消")]
    OperationCancelled,
    #[error("无效的请求头: {0}")]
    InvalidHeader(String),
}

/// 读取、解析或保存 TOML 配置时的错误。
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),
    #[error("TOML 解析错误: {0}")]
    TomlDe(#[from] toml::de::Error),
    #[error("TOML 序列化错误: {0}")]
    TomlSer(#[from] toml::ser::Error),
    #[error("配置文件未找到")]
    NotFound,
}

pub fn invalid(message: impl Into<String>) -> ClientError {
    ClientError::Other(message.into())
}
