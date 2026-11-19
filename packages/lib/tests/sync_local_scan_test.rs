use cloud139::domain::SyncEntryKind;
use cloud139::services::sync_service::{SyncScanOptions, scan_local};
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
            exclude: vec![],
        },
    )
    .unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].rel_path, "top.txt");
    assert_eq!(items[0].size, 3);
    assert!(items[0].mtime.is_some());
    assert_eq!(items[0].kind, SyncEntryKind::File);
}

#[test]
fn test_scan_local_recursive_uses_forward_slash_relative_paths() {
    let dir = tempdir().unwrap();
    write_file(&dir.path().join("nested/child.txt"), b"child");

    let items = scan_local(
        dir.path(),
        SyncScanOptions {
            recursive: true,
            exclude: vec![],
        },
    )
    .unwrap();

    let files: Vec<_> = items
        .iter()
        .filter(|i| i.kind == SyncEntryKind::File)
        .collect();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].rel_path, "nested/child.txt");
    assert!(
        items
            .iter()
            .any(|i| i.rel_path == "nested" && i.kind == SyncEntryKind::Directory)
    );
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
            exclude: vec![".git/**".to_string(), "target/**".to_string()],
        },
    )
    .unwrap();

    let files: Vec<_> = items
        .iter()
        .filter(|i| i.kind == SyncEntryKind::File)
        .collect();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].rel_path, "src/main.rs");
    assert!(
        items
            .iter()
            .any(|i| i.rel_path == "src" && i.kind == SyncEntryKind::Directory)
    );
}

#[test]
fn test_scan_local_recursive_keeps_empty_directories() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join("empty/sub")).unwrap();

    let items = scan_local(
        dir.path(),
        SyncScanOptions {
            recursive: true,
            exclude: vec![],
        },
    )
    .unwrap();

    assert!(
        items
            .iter()
            .any(|item| item.rel_path == "empty" && item.kind == SyncEntryKind::Directory)
    );
    assert!(
        items
            .iter()
            .any(|item| item.rel_path == "empty/sub" && item.kind == SyncEntryKind::Directory)
    );
}

#[test]
fn test_scan_local_excludes_empty_directories_with_glob() {
    // When target/** is excluded, the "target" directory itself should also be excluded
    // even if it's empty (no files under it match, but the dir node would still appear)
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join("target")).unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    write_file(&dir.path().join("src/main.rs"), b"fn main() {}");

    let items = scan_local(
        dir.path(),
        SyncScanOptions {
            recursive: true,
            exclude: vec!["target/**".to_string()],
        },
    )
    .unwrap();

    // "target" directory should NOT appear in results
    assert!(
        !items.iter().any(|item| item.rel_path == "target"),
        "Empty 'target' directory should be excluded when 'target/**' pattern is used, got: {:?}",
        items.iter().map(|i| &i.rel_path).collect::<Vec<_>>()
    );

    // "src" should still appear
    assert!(
        items
            .iter()
            .any(|item| item.rel_path == "src" && item.kind == SyncEntryKind::Directory)
    );
}
