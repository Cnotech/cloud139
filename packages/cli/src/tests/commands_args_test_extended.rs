#![allow(dead_code)]

mod commands_args_test_extended {
    use crate::cli::args::CpArgs;
    use crate::cli::args::DeleteArgs;
    use crate::cli::args::DownloadArgs;
    use crate::cli::args::ListArgs;
    use crate::cli::args::MkdirArgs;
    use crate::cli::args::MvArgs;
    use crate::cli::args::UploadArgs;
    use clap::Parser;

    #[test]
    fn test_list_args_default() {
        let args = ListArgs::parse_from(&["list"]);
        assert_eq!(args.path, "/");
        assert_eq!(args.page, 1);
        assert_eq!(args.page_size, 100);
    }

    #[test]
    fn test_list_args_with_path() {
        let args = ListArgs::parse_from(&["list", "/myfolder"]);
        assert_eq!(args.path, "/myfolder");
    }

    #[test]
    fn test_list_args_with_page() {
        let args = ListArgs::parse_from(&["list", "--page", "5"]);
        assert_eq!(args.page, 5);
    }

    #[test]
    fn test_list_args_with_page_size() {
        let args = ListArgs::parse_from(&["list", "-s", "50"]);
        assert_eq!(args.page_size, 50);
    }

    #[test]
    fn test_list_args_with_output() {
        let args = ListArgs::parse_from(&["list", "--output", "output.json"]);
        assert_eq!(args.output, Some("output.json".to_string()));
    }

    #[test]
    fn test_mkdir_args_default() {
        let args = MkdirArgs::parse_from(&["mkdir", "test"]);
        assert_eq!(args.path, "test");
    }

    #[test]
    fn test_delete_args_default() {
        let args = DeleteArgs::parse_from(&["delete", "file.txt"]);
        assert_eq!(args.path, "file.txt");
    }

    #[test]
    fn test_mv_args() {
        let args = MvArgs::parse_from(&["mv", "source.txt", "dest.txt"]);
        assert_eq!(args.source, vec!["source.txt"]);
        assert_eq!(args.target, "dest.txt");
    }

    #[test]
    fn test_mv_args_with_force() {
        let args = MvArgs::parse_from(&["mv", "source.txt", "dest.txt", "--force"]);
        assert!(args.force);
    }

    #[test]
    fn test_cp_args() {
        let args = CpArgs::parse_from(&["cp", "source.txt", "dest.txt"]);
        assert_eq!(args.source, vec!["source.txt"]);
        assert_eq!(args.target, "dest.txt");
    }

    #[test]
    fn test_cp_args_multiple_sources() {
        let args = CpArgs::parse_from(&["cp", "source1.txt", "source2.txt", "dest.txt"]);
        assert_eq!(args.source, vec!["source1.txt", "source2.txt"]);
        assert_eq!(args.target, "dest.txt");
    }

    #[test]
    fn test_download_args() {
        let args = DownloadArgs::parse_from(&["download", "file.txt"]);
        assert_eq!(args.remote_path, "file.txt");
    }

    #[test]
    fn test_download_args_with_local_path() {
        let args = DownloadArgs::parse_from(&["download", "file.txt", "output.txt"]);
        assert_eq!(args.remote_path, "file.txt");
        assert_eq!(args.local_path, Some("output.txt".to_string()));
    }

    #[test]
    fn test_upload_args() {
        let args = UploadArgs::parse_from(&["upload", "test.txt"]);
        assert_eq!(args.local_path, "test.txt");
        assert_eq!(args.remote_path, "/");
    }

    #[test]
    fn test_upload_args_with_force() {
        let args = UploadArgs::parse_from(&["upload", "test.txt", "--force"]);
        assert!(args.force);
    }
}
