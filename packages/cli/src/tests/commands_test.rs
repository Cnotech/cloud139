use crate::presentation::list::format_size;
use cloud139::client::api::upload::get_part_size;
use cloud139::models::list::parse_personal_time;

#[test]
fn test_format_size_bytes() {
    assert_eq!(format_size(0), "0 B");
    assert_eq!(format_size(1), "1 B");
    assert_eq!(format_size(512), "512 B");
    assert_eq!(format_size(1023), "1023 B");
}

#[test]
fn test_format_size_kilobytes() {
    assert_eq!(format_size(1024), "1.00 KB");
    assert_eq!(format_size(1536), "1.50 KB");
    assert_eq!(format_size(10240), "10.00 KB");
    assert_eq!(format_size(1048575), "1024.00 KB");
}

#[test]
fn test_format_size_megabytes() {
    assert_eq!(format_size(1048576), "1.00 MB");
    assert_eq!(format_size(1572864), "1.50 MB");
    assert_eq!(format_size(10485760), "10.00 MB");
}

#[test]
fn test_format_size_gigabytes() {
    assert_eq!(format_size(1073741824), "1.00 GB");
    assert_eq!(format_size(1610612736), "1.50 GB");
    assert_eq!(format_size(10737418240), "10.00 GB");
}

#[test]
fn test_parse_personal_time_empty() {
    assert_eq!(parse_personal_time(""), "");
}

#[test]
fn test_parse_personal_time_rfc3339() {
    let result = parse_personal_time("2024-01-15T10:30:00+08:00");
    assert!(result.contains("2024-01-15"));
}

#[test]
fn test_parse_personal_time_iso() {
    let result = parse_personal_time("2024-01-15T10:30:00.123");
    assert!(result.contains("2024-01-15"));
}

#[test]
fn test_parse_personal_time_plain() {
    let result = parse_personal_time("2024-01-15 10:30:00");
    assert!(result.contains("2024-01-15"));
}

#[test]
fn test_parse_personal_time_already_formatted() {
    let result = parse_personal_time("2024-01-15 10:30:00");
    assert_eq!(result, "2024-01-15 10:30:00");
}

#[test]
fn test_get_part_size_large_file() {
    let size_31gb: i64 = 31 * 1024 * 1024 * 1024;
    assert_eq!(get_part_size(size_31gb, 0), 512 * 1024 * 1024);
    let size_100gb: i64 = 100 * 1024 * 1024 * 1024;
    assert_eq!(get_part_size(size_100gb, 0), 512 * 1024 * 1024);
}

#[test]
fn test_get_part_size_small_file() {
    assert_eq!(get_part_size(1024 * 1024 * 10, 0), 100 * 1024 * 1024);
    assert_eq!(get_part_size(1024 * 1024 * 30, 0), 100 * 1024 * 1024);
}
