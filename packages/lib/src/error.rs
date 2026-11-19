use crate::{models::BaseResp, protocol};
use serde_json::Value;
use thiserror::Error;

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

#[derive(Debug, thiserror::Error)]
#[error("重试 {attempts} 次后仍失败")]
pub(crate) struct RetryError {
    pub(crate) attempts: u32,
}

pub fn invalid(message: impl Into<String>) -> ClientError {
    ClientError::Other(message.into())
}

pub(crate) fn format_api_error(operation: &str, message: &str, code: Option<&str>) -> String {
    match code {
        Some(code) => format!("{operation}: {message}（错误码: {code}）"),
        None => format!("{operation}: {message}"),
    }
}

pub(crate) fn upload_creation_error(response: &BaseResp, file_size: i64) -> ClientError {
    let code = response.code.as_deref();
    let message = format_api_error(
        "创建上传任务失败",
        response.message.as_deref().unwrap_or("未知错误"),
        code,
    );
    let message = if code == Some(protocol::UPLOAD_ENTITLEMENT_ERROR_CODE) {
        format!("{message}；文件大小 {file_size} 字节，请核对账号当前的单文件上传额度")
    } else {
        message
    };
    ClientError::Api(message)
}

pub(crate) fn json_api_error(operation: &str, response: &Value, fallback: &str) -> ClientError {
    ClientError::Api(format_api_error(
        operation,
        response["message"].as_str().unwrap_or(fallback),
        Some(response["code"].as_str().unwrap_or("未返回")),
    ))
}

pub(crate) fn require_success(response: BaseResp, fallback: &str) -> Result<(), ClientError> {
    if !response.success {
        return Err(ClientError::Api(
            response.message.unwrap_or_else(|| fallback.to_owned()),
        ));
    }
    Ok(())
}
