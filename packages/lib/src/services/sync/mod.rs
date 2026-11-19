//! 本地与个人云之间的文件同步。
//!
//! 用 [`SyncRequest::new`](plan::SyncRequest::new) 校验端点和选项，再用
//! [`SyncRequest::plan`](plan::SyncRequest::plan) 扫描两端并生成计划。
//! 可通过 [`SyncPlan::actions`](plan::SyncPlan::actions) 查看计划中的操作，再决定是否执行。

pub mod diff;
pub mod executor;
pub mod paths;
pub mod plan;
mod publish;
mod remote;
pub mod report;
pub mod scan;
mod verify;
