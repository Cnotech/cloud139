use clap::{CommandFactory, Parser};
use cloud139::application::services::sync_service::{parse_sync_endpoint, resolve_sync_direction};
use cloud139::cli::app::{Cli, Commands};
use cloud139::commands::sync::SyncArgs;
use cloud139::domain::{SyncDirection, SyncEndpoint};

#[test]
fn test_sync_args_parse_defaults() {
    let args = SyncArgs::try_parse_from(["sync", "./local", "cloud:/remote"]).unwrap();

    assert_eq!(args.src, "./local");
    assert_eq!(args.dest, "cloud:/remote");
    assert!(!args.recursive);
    assert!(!args.dry_run);
    assert!(!args.delete);
    assert!(args.exclude.is_empty());
    assert_eq!(args.jobs, 4);
}

#[test]
fn test_sync_args_parse_all_flags() {
    let args = SyncArgs::try_parse_from([
        "sync",
        "cloud:/photos",
        "./photos",
        "-r",
        "-n",
        "--delete",
        "--exclude",
        ".git",
        "--exclude",
        "target/**",
        "-j",
        "8",
    ])
    .unwrap();

    assert_eq!(args.src, "cloud:/photos");
    assert_eq!(args.dest, "./photos");
    assert!(args.recursive);
    assert!(args.dry_run);
    assert!(args.delete);
    assert_eq!(args.exclude, vec![".git", "target/**"]);
    assert_eq!(args.jobs, 8);
}

#[test]
fn test_sync_registered_on_root_cli() {
    let cli = Cli::try_parse_from(["cloud139", "sync", ".", "cloud:/project"]).unwrap();

    match cli.command {
        Commands::Sync(args) => {
            assert_eq!(args.src, ".");
            assert_eq!(args.dest, "cloud:/project");
        }
        _ => panic!("expected sync command"),
    }
}

#[test]
fn test_sync_rejects_zero_jobs() {
    let err = SyncArgs::try_parse_from(["sync", ".", "cloud:/project", "-j", "0"])
        .expect_err("jobs must reject zero");

    assert!(err.to_string().contains("jobs"));
}

#[test]
fn test_parse_cloud_endpoint_normalizes_path() {
    assert_eq!(
        parse_sync_endpoint("cloud:/remote/docs"),
        SyncEndpoint::Cloud("/remote/docs".to_string())
    );
    assert_eq!(
        parse_sync_endpoint("cloud:remote/docs"),
        SyncEndpoint::Cloud("/remote/docs".to_string())
    );
    assert_eq!(
        parse_sync_endpoint("cloud:"),
        SyncEndpoint::Cloud("/".to_string())
    );
}

#[test]
fn test_parse_local_endpoint_keeps_path() {
    assert_eq!(
        parse_sync_endpoint("./local/docs"),
        SyncEndpoint::Local(std::path::PathBuf::from("./local/docs"))
    );
}

#[test]
fn test_resolve_sync_direction_local_to_cloud() {
    let direction = resolve_sync_direction("./local", "cloud:/remote").unwrap();
    assert_eq!(direction, SyncDirection::LocalToCloud);
}

#[test]
fn test_resolve_sync_direction_cloud_to_local() {
    let direction = resolve_sync_direction("cloud:/remote", "./local").unwrap();
    assert_eq!(direction, SyncDirection::CloudToLocal);
}

#[test]
fn test_resolve_sync_direction_rejects_same_kind() {
    let local_err = resolve_sync_direction("./a", "./b").unwrap_err();
    assert!(local_err.to_string().contains("本地路径"));

    let cloud_err = resolve_sync_direction("cloud:/a", "cloud:/b").unwrap_err();
    assert!(cloud_err.to_string().contains("云端路径"));
}

use cloud139::commands::sync::{CommandExit, SyncArgs as CommandSyncArgs};

#[test]
fn test_command_exit_reports_configured_code() {
    let err = CommandExit::new(2, "bad path");
    assert_eq!(err.code(), 2);
    assert_eq!(err.to_string(), "bad path");
}

#[test]
fn test_sync_args_can_be_constructed_for_command_layer() {
    let args = CommandSyncArgs {
        src: "./local".to_string(),
        dest: "cloud:/remote".to_string(),
        recursive: true,
        dry_run: true,
        delete: false,
        exclude: vec!["target/**".to_string()],
        jobs: 4,
    };

    assert!(args.recursive);
    assert!(args.dry_run);
    assert_eq!(args.jobs, 4);
}

#[test]
fn test_sync_help_recursive_mentions_empty_dirs() {
    let help = SyncArgs::command().render_long_help().to_string();
    assert!(help.contains("空目录也会同步"));
}

#[test]
fn test_sync_help_delete_mentions_empty_dirs() {
    let help = SyncArgs::command().render_long_help().to_string();
    assert!(help.contains("或空目录"));
}
