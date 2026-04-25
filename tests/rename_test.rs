#![allow(dead_code)]

use cloud139::commands::rename;

#[test]
fn test_rename_args_validation() {
    let args = rename::RenameArgs {
        source: "/old.txt".to_string(),
        target: "new.txt".to_string(),
        force: false,
    };
    assert_eq!(args.source, "/old.txt");
    assert_eq!(args.target, "new.txt");
}
