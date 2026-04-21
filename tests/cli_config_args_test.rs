use clap::Parser;
use cloud139::cli::app::{Cli, Commands};

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
