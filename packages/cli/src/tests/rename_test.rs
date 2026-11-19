#![allow(dead_code)]

use crate::cli::args::RenameArgs;

#[test]
fn test_rename_args_validation() {
    let args = RenameArgs {
        source: "/old.txt".to_string(),
        target: "new.txt".to_string(),
        force: false,
    };
    assert_eq!(args.source, "/old.txt");
    assert_eq!(args.target, "new.txt");
}
