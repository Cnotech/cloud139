use crate::presentation::list::format_size;

#[test]
fn test_format_size_large_values() {
    assert_eq!(format_size(1073741824 * 5), "5.00 GB");
    assert_eq!(format_size(1073741824 * 10), "10.00 GB");
    assert_eq!(format_size(1073741824 * 100), "100.00 GB");
}
