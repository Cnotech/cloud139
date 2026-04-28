use cloud139::services::sync_service::{
    cloud_child_path, normalize_cloud_path, personal_item_to_file_entry,
    should_treat_personal_item_as_folder,
};
use cloud139::models::PersonalFileItem;

fn item(
    name: &str,
    size: Option<i64>,
    file_type: &str,
    updated_at: Option<&str>,
) -> PersonalFileItem {
    PersonalFileItem {
        file_id: Some(format!("id-{name}")),
        name: Some(name.to_string()),
        size,
        file_type: Some(file_type.to_string()),
        created_at: None,
        updated_at: updated_at.map(str::to_string),
        create_date: None,
        update_date: None,
        last_modified: None,
        thumbnail_urls: None,
        content_hash: None,
        content_hash_algorithm: None,
    }
}

#[test]
fn test_should_treat_personal_item_as_folder_accepts_known_folder_values() {
    assert!(should_treat_personal_item_as_folder(&item(
        "docs", None, "folder", None
    )));
    assert!(should_treat_personal_item_as_folder(&item(
        "docs", None, "dir", None
    )));
    assert!(should_treat_personal_item_as_folder(&item(
        "docs", None, "1", None
    )));
    assert!(!should_treat_personal_item_as_folder(&item(
        "readme.md",
        Some(3),
        "file",
        None
    )));
}

#[test]
fn test_cloud_child_path_joins_root_and_nested_paths() {
    assert_eq!(cloud_child_path("/", "docs"), "/docs");
    assert_eq!(cloud_child_path("/remote", "docs"), "/remote/docs");
    assert_eq!(
        cloud_child_path("/remote/docs", "readme.md"),
        "/remote/docs/readme.md"
    );
}

#[test]
fn test_personal_item_to_file_entry_uses_relative_path_and_mtime() {
    let entry = personal_item_to_file_entry(
        "docs",
        &item("readme.md", Some(42), "file", Some("2024-01-01T00:00:03Z")),
    )
    .unwrap();

    assert_eq!(entry.rel_path, "docs/readme.md");
    assert_eq!(entry.size, 42);
    assert_eq!(entry.mtime, Some(1704067203));
}

#[test]
fn test_normalize_cloud_path_maps_root_variants() {
    assert_eq!(normalize_cloud_path(""), "/");
    assert_eq!(normalize_cloud_path("/"), "/");
    assert_eq!(normalize_cloud_path("docs"), "/docs");
    assert_eq!(normalize_cloud_path("/docs"), "/docs");
    assert_eq!(normalize_cloud_path("docs/"), "/docs");
    assert_eq!(normalize_cloud_path("/docs/"), "/docs");
}

#[test]
fn test_cloud_child_path_root_produces_valid_absolute_paths() {
    assert_eq!(cloud_child_path("/", "readme.md"), "/readme.md");
    assert_eq!(cloud_child_path("", "readme.md"), "/readme.md");
}

#[test]
fn test_personal_item_to_file_entry_top_level_has_no_prefix_dot() {
    let entry = personal_item_to_file_entry(
        "",
        &item("top.txt", Some(10), "file", Some("2024-06-01T00:00:00Z")),
    )
    .unwrap();

    assert_eq!(entry.rel_path, "top.txt");
}
