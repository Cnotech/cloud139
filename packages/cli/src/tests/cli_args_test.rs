use clap::Parser;

use crate::cli::{Cli, Commands, DeleteArgs, DownloadArgs, ListArgs, MkdirArgs};

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

#[test]
fn test_cli_parse_global_config_before_subcommand() {
    let cli = Cli::parse_from([
        "cloud139",
        "--config",
        "/tmp/custom.toml",
        "login",
        "--token",
        "abc",
    ]);

    assert_eq!(cli.config.unwrap().to_string_lossy(), "/tmp/custom.toml");
    assert!(matches!(cli.command, Commands::Login(_)));
}

#[test]
fn test_cli_parse_global_config_after_subcommand() {
    let cli = Cli::parse_from([
        "cloud139",
        "login",
        "--token",
        "abc",
        "--config",
        "/tmp/custom.toml",
    ]);

    assert_eq!(cli.config.unwrap().to_string_lossy(), "/tmp/custom.toml");
    assert!(matches!(cli.command, Commands::Login(_)));
}

#[test]
fn recursive_cli_flags_parse() {
    assert!(ListArgs::parse_from(["ls", "-R"]).recursive);
    assert!(DownloadArgs::parse_from(["download", "-r", "/dir"]).recursive);
    assert!(MkdirArgs::parse_from(["mkdir", "-p", "/a/b"]).parents);
    let args = DeleteArgs::parse_from(["rm", "-r", "--yes", "/dir"]);
    assert!(args.recursive && args.yes);
}
