#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryKind {
    File,
    Folder,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileItem {
    pub name: String,
    pub kind: EntryKind,
    pub size: i64,
    pub modified: String,
    pub content_hash: Option<String>,
    pub content_hash_algorithm: Option<String>,
}
