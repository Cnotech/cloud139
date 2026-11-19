//! 配置类型、个人云校验和 TOML 文件读写。

mod file;
pub(crate) mod personal;

use self::personal::PERSONAL_STORAGE_TYPE;
use serde::{Deserialize, Serialize};

// 保留 CLI 和库调用方使用的配置文件操作入口。
#[doc(hidden)]
pub use self::file::{prepare_save_path, read_config, save_atomically};

/// Token 续期阈值，以毫秒为单位，等于 7 天。
pub const TOKEN_REFRESH_THRESHOLD_MS: i64 = 7 * 24 * 60 * 60 * 1000;

/// 个人云账号凭据和文件操作选项。
///
/// 默认配置使用 `personal_new` 存储类型。调用云端接口前，需提供有效的账号和凭据。
/// 此类型及其 TOML 表示包含凭据，应避免将其写入日志。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// 用于认证的 Base64 Token，省略 `Basic ` 前缀。
    pub authorization: String,
    /// 个人云账号。
    pub account: String,
    /// 存储类型，云端操作要求使用 `personal_new`。
    pub storage_type: String,
    /// 自定义上传分片大小（字节）。为 `0` 时按文件大小选用默认值。
    #[serde(default)]
    pub custom_upload_part_size: i64,
    /// 列表请求是否包含原图缩略图规格。
    #[serde(default)]
    pub use_large_thumbnail: bool,
    /// 已知的个人云服务地址。为 `None` 时通过路由接口解析。
    #[serde(default)]
    pub personal_cloud_host: Option<String>,
    /// 用于换取新 Token 的凭据。
    #[serde(default)]
    pub refresh_token: Option<String>,
    /// Token 的过期时间，以 Unix 时间戳毫秒表示。
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
    /// 判断是否达到续期条件。
    ///
    /// 剩余有效期不超过 [`TOKEN_REFRESH_THRESHOLD_MS`]，或者过期时间缺失时返回 `true`。
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
