use cloud139::application::services::sync_service::{SyncScanOptions, scan_local};
use std::fs;
use std::io::Write;
use tempfile::tempdir;

fn write_file(path: &std::path::Path, body: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    let mut file = fs::File::create(path).unwrap();
    file.write_all(body).unwrap();
}

#[test]
fn test_scan_local_non_recursive_only_reads_top_level_files() {
    let dir = tempdir().unwrap();
    write_file(&dir.path().join("top.txt"), b"top");
    write_file(&dir.path().join("nested/child.txt"), b"child");

    let items = scan_local(
        dir.path(),
        SyncScanOptions {
            recursive: false,
            checksum: false,
            exclude: vec![],
        },
    )
    .unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].rel_path, "top.txt");
    assert_eq!(items[0].size, 3);
    assert!(items[0].mtime.is_some());
    assert!(items[0].checksum.is_none());
}

#[test]
fn test_scan_local_recursive_uses_forward_slash_relative_paths() {
    let dir = tempdir().unwrap();
    write_file(&dir.path().join("nested/child.txt"), b"child");

    let items = scan_local(
        dir.path(),
        SyncScanOptions {
            recursive: true,
            checksum: false,
            exclude: vec![],
        },
    )
    .unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].rel_path, "nested/child.txt");
}

#[test]
fn test_scan_local_excludes_glob_patterns() {
    let dir = tempdir().unwrap();
    write_file(&dir.path().join(".git/config"), b"git");
    write_file(&dir.path().join("target/debug/app"), b"bin");
    write_file(&dir.path().join("src/main.rs"), b"fn main() {}");

    let items = scan_local(
        dir.path(),
        SyncScanOptions {
            recursive: true,
            checksum: false,
            exclude: vec![".git/**".to_string(), "target/**".to_string()],
        },
    )
    .unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].rel_path, "src/main.rs");
}

#[test]
fn test_scan_local_checksum_fills_sha1() {
    let dir = tempdir().unwrap();
    write_file(&dir.path().join("hash.txt"), b"hello");

    let items = scan_local(
        dir.path(),
        SyncScanOptions {
            recursive: false,
            checksum: true,
            exclude: vec![],
        },
    )
    .unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0].checksum.as_deref(),
        Some("aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d")
    );
}
