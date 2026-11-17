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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncEntryKind {
    File,
    Directory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    pub rel_path: String,
    pub size: u64,
    pub mtime: Option<i64>,
    pub kind: SyncEntryKind,
    /// 云端条目的 fileId（本地条目为 None）。执行阶段复用它，避免逐文件按路径解析。
    pub file_id: Option<String>,
    /// 云端条目的 contentHash（本地条目为 None）。--verify 差异判定用。
    pub content_hash: Option<String>,
    /// 云端条目的 contentHash 算法（本地条目为 None）。
    pub content_hash_algorithm: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    New,
    SizeOrTime,
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
        /// 目标父目录的云端 fileId（扫描得到时复用，None 时才按路径解析）。
        parent_file_id: Option<String>,
        /// 目标同名旧文件的云端 fileId（替换时先删它；新建为 None）。
        file_id: Option<String>,
    },
    Download {
        rel_path: String,
        remote_abs: String,
        local_abs: PathBuf,
        size: u64,
        change: ChangeKind,
        cloud_mtime: Option<i64>,
        /// 云端文件的 fileId（扫描得到时复用）。
        file_id: Option<String>,
        /// 云端父目录 fileId（用于兜底检查，避免按路径解析）。
        parent_file_id: Option<String>,
    },
    CreateDir {
        rel_path: String,
        target: SyncTarget,
        target_abs: String,
        /// 云端父目录 fileId（LocalToCloud 时复用）。
        parent_file_id: Option<String>,
    },
    Delete {
        rel_path: String,
        target: SyncTarget,
        target_abs: String,
        /// 待删除条目的云端 fileId（可为 None）。
        file_id: Option<String>,
        /// 条目类型；JSON 只统计文件。
        kind: SyncEntryKind,
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
    pub created_dirs: usize,
    pub failed: usize,
    pub bytes: u64,
}
