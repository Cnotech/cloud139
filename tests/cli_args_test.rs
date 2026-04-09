use clap::Parser;

use cloud139::cli::commands::list::ListArgs;

#[test]
fn test_list_args_parse_defaults() {
    let args = ListArgs::try_parse_from(["ls"]).unwrap();
    assert_eq!(args.path, "/");
    assert_eq!(args.page, 1);
    assert_eq!(args.page_size, 100);
    assert_eq!(args.output, None);
}

#[test]
fn test_list_args_parse_with_path() {
    let args = ListArgs::try_parse_from(["ls", "/my-folder"]).unwrap();
    assert_eq!(args.path, "/my-folder");
}

#[test]
fn test_list_args_parse_pagination() {
    let args = ListArgs::try_parse_from(["ls", "--page", "2", "-s", "50"]).unwrap();
    assert_eq!(args.page, 2);
    assert_eq!(args.page_size, 50);
}
