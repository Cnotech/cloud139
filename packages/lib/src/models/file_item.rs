//! 路径列表返回的文件与目录类型。

/// 列表条目的类型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryKind {
    File,
    Folder,
}

/// 列表中单个云端文件或目录的元数据。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileItem {
    pub name: String,
    pub kind: EntryKind,
    pub size: i64,
    pub modified: String,
    pub content_hash: Option<String>,
    pub content_hash_algorithm: Option<String>,
}
