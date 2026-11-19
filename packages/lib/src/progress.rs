//! 上传、下载和同步共用的传输进度接口，由调用方提供实现。

use std::sync::Arc;

/// 由调用方提供的传输进度接口，以字节计量。
pub trait TransferProgress: Send + Sync {
    /// 将已传输的字节数归零，例如在重新开始下载时。
    fn reset(&self);
    /// 设置预期传输的总字节数。
    fn set_length(&self, bytes: u64);
    /// 累加本次传输的字节数。
    fn inc(&self, bytes: u64);
    /// 结束当前进度显示。
    fn finish(&self);
}

/// 可在传输任务之间共享的进度实例。
pub type Progress = Arc<dyn TransferProgress>;
/// 根据条目的相对路径和传输大小（字节）创建可选进度实例的回调。
pub type ProgressFactory<'a> = dyn Fn(&str, u64) -> Option<Progress> + Sync + 'a;
