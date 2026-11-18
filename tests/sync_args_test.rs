use clap::{CommandFactory, Parser};
use cloud139::cli::app::{Cli, Commands};
use cloud139::commands::sync::SyncArgs;
use cloud139::domain::{SyncDirection, SyncEndpoint};
use cloud139::services::sync_service::{parse_sync_endpoint, sync_direction};

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
fn test_sync_direction_local_to_cloud() {
    let direction = sync_direction(
        &parse_sync_endpoint("./local"),
        &parse_sync_endpoint("cloud:/remote"),
    )
    .unwrap();
    assert_eq!(direction, SyncDirection::LocalToCloud);
}

#[test]
fn test_sync_direction_cloud_to_local() {
    let direction = sync_direction(
        &parse_sync_endpoint("cloud:/remote"),
        &parse_sync_endpoint("./local"),
    )
    .unwrap();
    assert_eq!(direction, SyncDirection::CloudToLocal);
}

#[test]
fn test_sync_direction_rejects_same_kind() {
    let local_err =
        sync_direction(&parse_sync_endpoint("./a"), &parse_sync_endpoint("./b")).unwrap_err();
    assert!(local_err.to_string().contains("本地路径"));

    let cloud_err = sync_direction(
        &parse_sync_endpoint("cloud:/a"),
        &parse_sync_endpoint("cloud:/b"),
    )
    .unwrap_err();
    assert!(cloud_err.to_string().contains("云端路径"));
}

use cloud139::commands::CommandExit;
use cloud139::commands::sync::SyncArgs as CommandSyncArgs;

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
        verify: false,
        json: false,
    };

    assert!(args.recursive);
    assert!(args.dry_run);
    assert_eq!(args.jobs, 4);
}

#[test]
fn test_json_is_available_with_or_without_verify() {
    let verified =
        SyncArgs::try_parse_from(["sync", ".", "cloud:/backup", "--verify", "--json"]).unwrap();
    assert_eq!(verified.jobs, 1);
    assert!(verified.verify && verified.json);
    let ordinary = SyncArgs::try_parse_from(["sync", "cloud:/backup", ".", "--json"]).unwrap();
    assert_eq!(ordinary.jobs, 4);
    assert!(!ordinary.verify && ordinary.json);
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

#[test]
fn sync_enforces_the_concurrency_limit() {
    let args = SyncArgs::try_parse_from(["sync", "local", "cloud:/", "--jobs", "32"]).unwrap();
    assert_eq!(args.jobs, 32);

    for jobs in ["33".to_owned(), usize::MAX.to_string()] {
        let error = SyncArgs::try_parse_from(["sync", "local", "cloud:/", "--jobs", &jobs])
            .expect_err("jobs above the supported range must be rejected");
        assert!(error.to_string().contains("1 和 32"));
    }
}
