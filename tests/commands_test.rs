use cloud139::application::services::mkdir_service;
use cloud139::application::services::upload_service;
use cloud139::commands::list;

#[test]
fn test_parse_path_root() {
    let result = mkdir_service::parse_path("/test");
    assert!(result.is_ok());
    let (parent, name) = result.unwrap();
    assert_eq!(parent, "/");
    assert_eq!(name, "test");
}

#[test]
fn test_parse_path_nested() {
    let result = mkdir_service::parse_path("/parent/child");
    assert!(result.is_ok());
    let (parent, name) = result.unwrap();
    assert_eq!(parent, "/parent");
    assert_eq!(name, "child");
}

#[test]
fn test_parse_path_deep_nested() {
    let result = mkdir_service::parse_path("/a/b/c/d");
    assert!(result.is_ok());
    let (parent, name) = result.unwrap();
    assert_eq!(parent, "/a/b/c");
    assert_eq!(name, "d");
}

#[test]
fn test_parse_path_single_name() {
    let result = mkdir_service::parse_path("test");
    assert!(result.is_ok());
    let (parent, name) = result.unwrap();
    assert_eq!(parent, "/");
    assert_eq!(name, "test");
}

#[test]
fn test_parse_path_empty() {
    let result = mkdir_service::parse_path("");
    assert!(result.is_err());
}

#[test]
fn test_parse_path_only_slash() {
    let result = mkdir_service::parse_path("/");
    assert!(result.is_err());
}

#[test]
fn test_parse_path_whitespace() {
    let result = mkdir_service::parse_path("  ");
    assert!(result.is_err());
}

#[test]
fn test_parse_path_with_spaces() {
    let result = mkdir_service::parse_path("/my folder/file");
    assert!(result.is_ok());
    let (parent, name) = result.unwrap();
    assert_eq!(parent, "/my folder");
    assert_eq!(name, "file");
}

#[test]
fn test_format_size_bytes() {
    assert_eq!(list::format_size(0), "0 B");
    assert_eq!(list::format_size(1), "1 B");
    assert_eq!(list::format_size(512), "512 B");
    assert_eq!(list::format_size(1023), "1023 B");
}

#[test]
fn test_format_size_kilobytes() {
    assert_eq!(list::format_size(1024), "1.00 KB");
    assert_eq!(list::format_size(1536), "1.50 KB");
    assert_eq!(list::format_size(10240), "10.00 KB");
    assert_eq!(list::format_size(1048575), "1024.00 KB");
}

#[test]
fn test_format_size_megabytes() {
    assert_eq!(list::format_size(1048576), "1.00 MB");
    assert_eq!(list::format_size(1572864), "1.50 MB");
    assert_eq!(list::format_size(10485760), "10.00 MB");
}

#[test]
fn test_format_size_gigabytes() {
    assert_eq!(list::format_size(1073741824), "1.00 GB");
    assert_eq!(list::format_size(1610612736), "1.50 GB");
    assert_eq!(list::format_size(10737418240), "10.00 GB");
}

#[test]
fn test_parse_personal_time_empty() {
    assert_eq!(list::parse_personal_time(""), "");
}

#[test]
fn test_parse_personal_time_rfc3339() {
    let result = list::parse_personal_time("2024-01-15T10:30:00+08:00");
    assert!(result.contains("2024-01-15"));
}

#[test]
fn test_parse_personal_time_iso() {
    let result = list::parse_personal_time("2024-01-15T10:30:00.123");
    assert!(result.contains("2024-01-15"));
}

#[test]
fn test_parse_personal_time_plain() {
    let result = list::parse_personal_time("2024-01-15 10:30:00");
    assert!(result.contains("2024-01-15"));
}

#[test]
fn test_parse_personal_time_already_formatted() {
    let result = list::parse_personal_time("2024-01-15 10:30:00");
    assert_eq!(result, "2024-01-15 10:30:00");
}

#[test]
fn test_get_part_size_large_file() {
    let size_31gb: i64 = 31 * 1024 * 1024 * 1024;
    assert_eq!(upload_service::get_part_size(size_31gb, 0), 512 * 1024 * 1024);
    let size_100gb: i64 = 100 * 1024 * 1024 * 1024;
    assert_eq!(upload_service::get_part_size(size_100gb, 0), 512 * 1024 * 1024);
}

#[test]
fn test_get_part_size_small_file() {
    assert_eq!(
        upload_service::get_part_size(1024 * 1024 * 10, 0),
        100 * 1024 * 1024
    );
    assert_eq!(
        upload_service::get_part_size(1024 * 1024 * 30, 0),
        100 * 1024 * 1024
    );
}
