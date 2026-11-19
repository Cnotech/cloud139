use crate::config::Config;
use crate::error::ClientError;
use std::borrow::Borrow;
use std::ops::Deref;

pub(super) const PERSONAL_STORAGE_TYPE: &str = "personal_new";

/// 已通过个人云校验的配置，内部调用可继续使用其只读引用。
#[derive(Clone, Copy)]
pub(crate) struct PersonalConfig<T>(T);

impl<T: Borrow<Config>> PersonalConfig<T> {
    pub(crate) fn new(config: T) -> Result<Self, ClientError> {
        config.borrow().require_personal()?;
        Ok(Self(config))
    }
}

impl PersonalConfig<Config> {
    pub(crate) fn as_ref(&self) -> PersonalConfig<&Config> {
        PersonalConfig(&self.0)
    }

    pub(crate) fn set_cloud_host(&mut self, host: String) {
        self.0.personal_cloud_host = Some(host);
    }

    pub(crate) fn into_inner(self) -> Config {
        self.0
    }
}

impl<T: Borrow<Config>> Deref for PersonalConfig<T> {
    type Target = Config;

    fn deref(&self) -> &Config {
        self.0.borrow()
    }
}

impl Config {
    /// 检查存储类型是否为 `personal_new`。
    pub fn require_personal(&self) -> Result<(), ClientError> {
        if self.storage_type == PERSONAL_STORAGE_TYPE {
            Ok(())
        } else {
            Err(ClientError::UnsupportedStorage)
        }
    }
}
