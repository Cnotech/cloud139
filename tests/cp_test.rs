#![allow(dead_code)]

use cloud139::commands::cp;

#[test]
fn test_cp_args_validation() {
    let args = cp::CpArgs {
        source: "/source.txt".to_string(),
        target: "/target/".to_string(),
        force: false,
    };
    assert_eq!(args.source, "/source.txt");
    assert_eq!(args.target, "/target/");
    assert!(!args.force);
}

#[test]
fn test_cp_args_with_force() {
    let args = cp::CpArgs {
        source: "/source.txt".to_string(),
        target: "/target/".to_string(),
        force: true,
    };
    assert!(args.force);
}
