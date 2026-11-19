use crate::client::api::personal::client::ListingRetry;

/// 同步读请求最多重试 4 次，退避 2/4/8/16 秒。
///
/// 用于列表、下载和路径解析。
pub(super) const SYNC_RETRIES: u32 = 4;
pub(super) const SYNC_BACKOFF_SECS: u64 = 2;

pub(in crate::services) const SYNC_LIST_RETRY: ListingRetry = ListingRetry::Transient {
    retries: SYNC_RETRIES,
    backoff_secs: SYNC_BACKOFF_SECS,
    timeout: None,
};
