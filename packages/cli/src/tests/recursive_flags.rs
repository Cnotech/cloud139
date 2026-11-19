#[test]
fn recursive_cli_flags_parse() {
    use clap::Parser;
    assert!(crate::cli::args::ListArgs::parse_from(["ls", "-R"]).recursive);
    assert!(crate::cli::args::DownloadArgs::parse_from(["download", "-r", "/dir"]).recursive);
    assert!(crate::cli::args::MkdirArgs::parse_from(["mkdir", "-p", "/a/b"]).parents);
    let args = crate::cli::args::DeleteArgs::parse_from(["rm", "-r", "--yes", "/dir"]);
    assert!(args.recursive && args.yes);
}
