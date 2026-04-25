pub mod cp;
pub mod delete;
pub mod download;
pub mod list;
pub mod login;
pub mod mkdir;
pub mod mv;
pub mod rename;
pub mod sync;
pub mod upload;

use std::fmt;

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
