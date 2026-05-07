#![allow(dead_code)]

use clap::Parser;
use cloud139::commands::cp::CpArgs;
use cloud139::commands::delete::DeleteArgs;
use cloud139::commands::mv::MvArgs;

#[test]
fn test_cp_args_parse() {
    let args = CpArgs::try_parse_from(["cp", "source.txt", "/dest/"]).unwrap();
    assert_eq!(args.source, vec!["source.txt"]);
    assert_eq!(args.target, "/dest/");
    assert!(!args.force);
}

#[test]
fn test_cp_args_parse_multiple_sources() {
    let args = CpArgs::try_parse_from(["cp", "file1.txt", "file2.txt", "/dest/"]).unwrap();
    assert_eq!(args.source, vec!["file1.txt", "file2.txt"]);
    assert_eq!(args.target, "/dest/");
    assert!(!args.force);
}

#[test]
fn test_cp_args_parse_with_force() {
    let args = CpArgs::try_parse_from(["cp", "source.txt", "/dest/", "--force"]).unwrap();
    assert_eq!(args.source, vec!["source.txt"]);
    assert!(args.force);
}

#[test]
fn test_delete_args_parse() {
    let args = DeleteArgs::try_parse_from(["delete", "/path/to/file"]).unwrap();
    assert_eq!(args.path, "/path/to/file");
    assert!(!args.yes);
    assert!(!args.permanent);
}

#[test]
fn test_delete_args_parse_with_yes() {
    let args = DeleteArgs::try_parse_from(["delete", "/path/to/file", "--yes"]).unwrap();
    assert!(args.yes);
}

#[test]
fn test_delete_args_parse_with_permanent() {
    let args = DeleteArgs::try_parse_from(["delete", "/path/to/file", "--permanent"]).unwrap();
    assert!(args.permanent);
}

#[test]
fn test_mv_args_parse_single_source() {
    let args = MvArgs::try_parse_from(["mv", "source.txt", "/dest/"]).unwrap();
    assert_eq!(args.source.len(), 1);
    assert_eq!(args.source[0], "source.txt");
    assert_eq!(args.target, "/dest/");
    assert!(!args.force);
}

#[test]
fn test_mv_args_parse_multiple_sources() {
    let args = MvArgs::try_parse_from(["mv", "file1.txt", "file2.txt", "/dest/"]).unwrap();
    assert_eq!(args.source.len(), 2);
    assert_eq!(args.source[0], "file1.txt");
    assert_eq!(args.source[1], "file2.txt");
    assert_eq!(args.target, "/dest/");
}

#[test]
fn test_mv_args_parse_with_force() {
    let args = MvArgs::try_parse_from(["mv", "source.txt", "/dest/", "--force"]).unwrap();
    assert!(args.force);
}
