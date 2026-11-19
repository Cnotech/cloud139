use cloud139::config::ConfigError;
use std::{fmt, io};
use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum ConfigWriteError {
    #[error(transparent)]
    File(#[from] ConfigError),
    #[error("配置正在被另一个进程更新")]
    Busy,
    #[error("配置已被其他操作修改，放弃写回续期结果")]
    Changed,
}

impl From<io::Error> for ConfigWriteError {
    fn from(error: io::Error) -> Self {
        Self::File(error.into())
    }
}

/// 带自定义退出码的命令错误。
#[derive(Debug)]
pub struct CommandExit {
    code: i32,
    message: String,
}

impl CommandExit {
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn code(&self) -> i32 {
        self.code
    }
}

impl fmt::Display for CommandExit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for CommandExit {}
