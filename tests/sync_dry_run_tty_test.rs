// tests/sync_dry_run_tty_test.rs
//
// 验证 dry-run 文件列表格式化逻辑不依赖 is_tty 状态。
// 这里通过直接调用 format_action_line 检查前缀，而不是模拟 TTY 环境。

use cloud139::application::services::sync_service::format_action_line;
use cloud139::domain::{ChangeKind, SyncAction, SyncTarget};
use std::path::PathBuf;

#[test]
fn dry_run_upload_line_has_prefix() {
    let action = SyncAction::Upload {
        rel_path: "docs/readme.md".to_string(),
        local_abs: PathBuf::from("/local/docs/readme.md"),
        remote_abs: "cloud:/docs/readme.md".to_string(),
        size: 100,
        change: ChangeKind::New,
    };
    // format_action_line(action, dry_run=true) 应以 "(DRY RUN)" 开头
    let line = format_action_line(&action, true);
    assert!(line.starts_with("(DRY RUN)"), "got: {line}");
}

#[test]
fn dry_run_delete_line_has_prefix() {
    let action = SyncAction::Delete {
        rel_path: "docs/old.txt".to_string(),
        target: SyncTarget::Cloud,
        target_abs: "cloud:/docs/old.txt".to_string(),
    };
    let line = format_action_line(&action, true);
    assert!(line.starts_with("(DRY RUN)"), "got: {line}");
}