use cloud139::error::ConfigError;
use std::io;
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
