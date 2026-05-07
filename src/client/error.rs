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
    Config(#[from] crate::config::ConfigError),
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
    #[error("家庭云暂不支持批量移动")]
    UnsupportedFamilyBatchMove,
    #[error("群组云暂不支持批量移动")]
    UnsupportedGroupBatchMove,
    #[error("家庭云暂不支持批量复制")]
    UnsupportedFamilyBatchCopy,
    #[error("群组云暂不支持批量复制")]
    UnsupportedGroupBatchCopy,
    #[error("家庭云不支持重命名文件夹")]
    UnsupportedFamilyRenameFolder,
    #[error("不支持下载目录，请使用 ls 命令查看目录内容")]
    UnsupportedDownloadDirectory,
    #[error("无效的文件路径")]
    InvalidFilePath,
    #[error("操作被取消")]
    OperationCancelled,
    #[error("无效的请求头: {0}")]
    InvalidHeader(String),
}
