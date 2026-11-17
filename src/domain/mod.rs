// src/domain/mod.rs
pub mod file_item;
pub mod storage_type;
pub mod sync_item;
pub mod upload_policy;

pub use file_item::{EntryKind, FileItem};
pub use storage_type::StorageType;
pub use sync_item::{
    ChangeKind, FileEntry, SyncAction, SyncDirection, SyncEndpoint, SyncEntryKind, SyncSummary,
    SyncTarget,
};
pub use upload_policy::PERSONAL_UPLOAD_MAX_BYTES;
