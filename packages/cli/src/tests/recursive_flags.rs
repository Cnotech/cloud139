#[test]
fn recursive_cli_flags_parse() {
    use clap::Parser;
    assert!(crate::commands::list::ListArgs::parse_from(["ls", "-R"]).recursive);
    assert!(
        crate::commands::download::DownloadArgs::parse_from(["download", "-r", "/dir"]).recursive
    );
    assert!(crate::commands::mkdir::MkdirArgs::parse_from(["mkdir", "-p", "/a/b"]).parents);
    let args = crate::commands::delete::DeleteArgs::parse_from(["rm", "-r", "--yes", "/dir"]);
    assert!(args.recursive && args.yes);
}
