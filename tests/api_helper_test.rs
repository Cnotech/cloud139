#![allow(dead_code)]

use cloud139::client::api::{get_parent_id, parse_path_segments};

#[test]
fn test_parse_path_segments_simple() {
    let segments = parse_path_segments("folder/file.txt");
    assert_eq!(segments, vec!["folder", "file.txt"]);
}

#[test]
fn test_parse_path_segments_with_leading_slash() {
    let segments = parse_path_segments("/folder/file.txt");
    assert_eq!(segments, vec!["folder", "file.txt"]);
}

#[test]
fn test_parse_path_segments_multiple_slashes() {
    let segments = parse_path_segments("///folder///file.txt///");
    assert_eq!(segments, vec!["folder", "file.txt"]);
}

#[test]
fn test_parse_path_segments_empty_parts() {
    let segments = parse_path_segments("folder//file.txt");
    assert_eq!(segments, vec!["folder", "file.txt"]);
}

#[test]
fn test_parse_path_segments_single_file() {
    let segments = parse_path_segments("file.txt");
    assert_eq!(segments, vec!["file.txt"]);
}

#[test]
fn test_parse_path_segments_empty() {
    let segments = parse_path_segments("");
    assert!(segments.is_empty());
}

#[test]
fn test_parse_path_segments_root() {
    let segments = parse_path_segments("/");
    assert!(segments.is_empty());
}

#[test]
fn test_get_parent_id_empty() {
    let parent = get_parent_id("");
    assert_eq!(parent, "/");
}

#[test]
fn test_get_parent_id_non_empty() {
    let parent = get_parent_id("123");
    assert_eq!(parent, "123");
}
