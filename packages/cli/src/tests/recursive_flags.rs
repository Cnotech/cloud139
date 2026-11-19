#[test]
fn recursive_cli_flags_parse() {
    use clap::Parser;
    assert!(crate::cli::ListArgs::parse_from(["ls", "-R"]).recursive);
    assert!(crate::cli::DownloadArgs::parse_from(["download", "-r", "/dir"]).recursive);
    assert!(crate::cli::MkdirArgs::parse_from(["mkdir", "-p", "/a/b"]).parents);
    let args = crate::cli::DeleteArgs::parse_from(["rm", "-r", "--yes", "/dir"]);
    assert!(args.recursive && args.yes);
}
