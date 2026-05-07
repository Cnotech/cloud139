use crate::domain::SyncSummary;
use crate::success;

/// 打印同步结果摘要。
pub fn print_summary(summary: &SyncSummary) {
    let mut parts = Vec::new();
    if summary.transferred > 0 {
        parts.push(format!("{} 个文件传输", summary.transferred));
    }
    if summary.created_dirs > 0 {
        parts.push(format!("{} 个目录创建", summary.created_dirs));
    }
    if summary.deleted > 0 {
        parts.push(format!("{} 个删除", summary.deleted));
    }
    if summary.skipped > 0 {
        parts.push(format!("{} 个跳过", summary.skipped));
    }
    if summary.failed > 0 {
        parts.push(format!("{} 个失败", summary.failed));
    }
    if parts.is_empty() {
        success!("同步完成: 无变化");
    } else {
        success!("同步完成: {}", parts.join(", "));
    }
}
