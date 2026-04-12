use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncEndpoint {
    Local(PathBuf),
    Cloud(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncDirection {
    LocalToCloud,
    CloudToLocal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    pub rel_path: String,
    pub size: u64,
    pub mtime: Option<i64>,
    pub checksum: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    New,
    SizeOrTime,
    Checksum,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncTarget {
    Local,
    Cloud,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncAction {
    Upload {
        rel_path: String,
        local_abs: PathBuf,
        remote_abs: String,
        size: u64,
        change: ChangeKind,
    },
    Download {
        rel_path: String,
        remote_abs: String,
        local_abs: PathBuf,
        size: u64,
        change: ChangeKind,
        cloud_mtime: Option<i64>,
    },
    Delete {
        rel_path: String,
        target: SyncTarget,
        target_abs: String,
    },
    Skip {
        rel_path: String,
    },
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SyncSummary {
    pub transferred: usize,
    pub skipped: usize,
    pub deleted: usize,
    pub failed: usize,
    pub bytes: u64,
}
