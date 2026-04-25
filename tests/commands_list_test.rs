#![allow(dead_code)]

mod commands_list_test {
    use cloud139::presentation::list_renderer::format_size;
    use cloud139::utils::parse_personal_time;

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
    }

    #[test]
    fn test_parse_personal_time_empty() {
        assert_eq!(parse_personal_time(""), "");
    }

    #[test]
    fn test_parse_personal_time_rfc3339() {
        let result = parse_personal_time("2024-01-01T00:00:00Z");
        assert!(result.contains("2024-01-01"));
    }

    #[test]
    fn test_parse_personal_time_with_millis() {
        let result = parse_personal_time("2024-01-01T12:30:45.123456Z");
        assert!(result.contains("2024-01-01"));
        assert!(result.contains("12:30:45"));
    }

    #[test]
    fn test_parse_personal_time_other_format() {
        let result = parse_personal_time("2024-01-01 12:30:45");
        assert!(result.contains("2024-01-01"));
    }

    #[test]
    fn test_parse_personal_time_unknown_format() {
        let result = parse_personal_time("unknown");
        assert_eq!(result, "unknown");
    }
}
