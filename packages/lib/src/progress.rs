//! 上传、下载和同步共用的传输进度接口，由调用方提供实现。

use std::sync::Arc;

/// 应用可提供的传输进度接口。
pub trait TransferProgress: Send + Sync {
    fn reset(&self);
    fn set_length(&self, bytes: u64);
    fn inc(&self, bytes: u64);
    fn finish(&self);
}

pub type Progress = Arc<dyn TransferProgress>;
pub type ProgressFactory<'a> = dyn Fn(&str, u64) -> Option<Progress> + Sync + 'a;
