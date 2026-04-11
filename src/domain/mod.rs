// src/domain/mod.rs
pub mod file_item;
pub mod sync_item;

pub use file_item::{EntryKind, FileItem};
pub use sync_item::{
    ChangeKind, FileEntry, SyncAction, SyncDirection, SyncEndpoint, SyncSummary, SyncTarget,
};
