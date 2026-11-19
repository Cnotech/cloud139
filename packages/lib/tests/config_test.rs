use cloud139::config::{Config, save_atomically};
use cloud139::error::ConfigError;
use std::{fs, io};

#[test]
fn atomic_save_without_overwrite_keeps_existing_config() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let original = Config {
        account: "original".into(),
        ..Default::default()
    };
    save_atomically(&original, &path, false).unwrap();

    let replacement = Config::default();
    assert!(matches!(
        save_atomically(&replacement, &path, false),
        Err(ConfigError::Io(error)) if error.kind() == io::ErrorKind::AlreadyExists
    ));
    assert_eq!(Config::load_from_path(&path).unwrap().account, "original");
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}
