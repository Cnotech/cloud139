use std::fmt;

/// 带自定义退出码的命令错误。
#[derive(Debug)]
pub(crate) struct CommandExit {
    code: i32,
    message: String,
}

impl CommandExit {
    pub(crate) fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub(crate) fn code(&self) -> i32 {
        self.code
    }
}

impl fmt::Display for CommandExit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for CommandExit {}
