use cloud139::application::services::mkdir_service;
use cloud139::presentation::renderers::list_renderer::format_size;

#[test]
fn test_format_size_large_values() {
    assert_eq!(format_size(1073741824 * 5), "5.00 GB");
    assert_eq!(format_size(1073741824 * 10), "10.00 GB");
    assert_eq!(format_size(1073741824 * 100), "100.00 GB");
}

#[test]
fn test_mkdir_parse_path_edge_cases_new() {
    let result = mkdir_service::parse_path("/folder/subfolder");
    assert!(result.is_ok());
    let (parent, name) = result.unwrap();
    assert_eq!(parent, "/folder");
    assert_eq!(name, "subfolder");

    let result2 = mkdir_service::parse_path("/single");
    assert!(result2.is_ok());
    let (parent2, name2) = result2.unwrap();
    assert_eq!(parent2, "/");
    assert_eq!(name2, "single");
}
