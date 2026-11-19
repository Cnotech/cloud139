//! 配置类型、个人云校验和 TOML 文件读写。

mod file;
pub(crate) mod personal;

use self::personal::PERSONAL_STORAGE_TYPE;
use serde::{Deserialize, Serialize};

// 保留 CLI 和库调用方使用的配置文件操作入口。
#[doc(hidden)]
pub use self::file::{prepare_save_path, read_config, save_atomically};

/// 剩余有效期不超过这个时长时，在下一次命令启动时续期 token。
///
/// 这是「何时续期」的唯一阈值：续期接口每次滚动发放 30 天有效的新 token，
/// 留 7 天余量可以容忍几次失败重试。
pub const TOKEN_REFRESH_THRESHOLD_MS: i64 = 7 * 24 * 60 * 60 * 1000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub authorization: String,
    pub account: String,
    pub storage_type: String,
    #[serde(default)]
    pub custom_upload_part_size: i64,
    #[serde(default)]
    pub use_large_thumbnail: bool,
    #[serde(default)]
    pub personal_cloud_host: Option<String>,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub token_expire_time: Option<i64>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            authorization: String::new(),
            account: String::new(),
            storage_type: PERSONAL_STORAGE_TYPE.to_owned(),
            custom_upload_part_size: 0,
            use_large_thumbnail: false,
            personal_cloud_host: None,
            refresh_token: None,
            token_expire_time: None,
        }
    }
}

impl Config {
    /// 下一次命令启动时是否需要续期（唯一判断）。
    /// 剩余有效期不超过阈值，或没有过期时间时返回 true。
    pub fn is_token_refresh_due(&self) -> bool {
        self.is_token_refresh_due_at(chrono::Utc::now().timestamp_millis())
    }

    fn is_token_refresh_due_at(&self, now_ms: i64) -> bool {
        match self.token_expire_time {
            Some(expire_time) => expire_time - now_ms <= TOKEN_REFRESH_THRESHOLD_MS,
            None => true,
        }
    }
}

#[cfg(test)]
mod refresh_tests {
    use super::*;

    #[test]
    fn refresh_threshold_includes_exactly_seven_days() {
        let now = 1_800_000_000_000;
        let seven_days = 7 * 86_400_000;
        for (remaining, expected) in [
            (None, true),
            (Some(-1), true),
            (Some(0), true),
            (Some(seven_days - 1), true),
            (Some(seven_days), true),
            (Some(seven_days + 1), false),
            (Some(30 * 86_400_000), false),
        ] {
            let config = Config {
                token_expire_time: remaining.map(|ms| now + ms),
                ..Default::default()
            };
            assert_eq!(
                config.is_token_refresh_due_at(now),
                expected,
                "{remaining:?}"
            );
        }
    }
}
