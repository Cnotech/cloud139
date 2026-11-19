//! 本地与个人云之间的文件同步。

pub mod diff;
pub mod executor;
pub mod paths;
pub mod plan;
pub(super) mod remote;
pub mod report;
pub mod scan;
mod verify;
