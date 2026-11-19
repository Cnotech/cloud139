#![allow(dead_code)]

use crate::cli::MkdirArgs;

#[test]
fn test_mkdir_args_defaults() {
    let args = MkdirArgs {
        parents: false,
        path: "/test".to_string(),
    };
    assert_eq!(args.path, "/test");
}
