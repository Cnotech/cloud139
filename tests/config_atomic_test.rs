use cloud139::config::Config;
use std::fs;
use std::sync::{Arc, Barrier};

fn config(account: &str) -> Config {
    Config {
        account: account.into(),
        authorization: "test-authorization".repeat(1000),
        ..Default::default()
    }
}

#[test]
fn concurrent_readers_see_only_complete_configurations() {
    let dir = tempfile::tempdir().unwrap();
    let path = Arc::new(dir.path().join("config.toml"));
    config("first").save_to(&path).unwrap();
    let barrier = Arc::new(Barrier::new(2));
    let reader_path = path.clone();
    let reader_barrier = barrier.clone();
    let reader = std::thread::spawn(move || {
        reader_barrier.wait();
        for _ in 0..300 {
            let content = fs::read_to_string(&*reader_path).unwrap();
            let parsed: Config = toml::from_str(&content).unwrap();
            assert!(parsed.account == "first" || parsed.account == "second");
            assert_eq!(parsed.authorization, "test-authorization".repeat(1000));
        }
    });
    barrier.wait();
    for n in 0..100 {
        config(if n % 2 == 0 { "second" } else { "first" })
            .save_to(&path)
            .unwrap();
    }
    reader.join().unwrap();
    // 保存后只保留配置本身，不产生锁文件或遗留临时文件。
    let entries: Vec<_> = fs::read_dir(dir.path()).unwrap().collect();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].as_ref().unwrap().file_name(), "config.toml");
}

#[cfg(unix)]
#[test]
fn atomic_save_restricts_permissions_and_preserves_symlink() {
    use std::os::unix::{fs::PermissionsExt, fs::symlink};
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("config.toml");
    let link = dir.path().join("alias.toml");
    fs::write(&target, toml::to_string(&config("first")).unwrap()).unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o644)).unwrap();
    symlink(&target, &link).unwrap();
    config("second").save_to(&link).unwrap();
    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    let parsed: Config = toml::from_str(&fs::read_to_string(&target).unwrap()).unwrap();
    assert_eq!(parsed.account, "second");
    assert_eq!(
        fs::metadata(&target).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[test]
fn failed_atomic_replace_keeps_original_target_and_cleans_tempfile() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("config.toml");
    // 不能将临时文件原子替换到目录上：失败后原目录及其内容必须保留。
    fs::create_dir(&target).unwrap();
    fs::write(target.join("original"), "unchanged").unwrap();
    assert!(config("second").save_to(&target).is_err());
    assert_eq!(
        fs::read_to_string(target.join("original")).unwrap(),
        "unchanged"
    );
    assert!(fs::read_dir(dir.path()).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".cloud139-config-")
    }));
}

#[test]
fn concurrent_first_saves_never_expose_empty_config_or_leave_lock_files() {
    use cloud139::config::ConfigError;
    let dir = tempfile::tempdir().unwrap();
    let path = Arc::new(dir.path().join("config.toml"));
    let barrier = Arc::new(Barrier::new(9));
    let writers: Vec<_> = (0..8)
        .map(|n| {
            let path = path.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                let config = config(&format!("writer-{n}"));
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
                loop {
                    match config.save_to(&path) {
                        Ok(()) => return,
                        Err(ConfigError::Busy) if std::time::Instant::now() < deadline => {
                            std::thread::sleep(std::time::Duration::from_millis(1));
                        }
                        Err(err) => panic!("save failed: {err}"),
                    }
                }
            })
        })
        .collect();
    barrier.wait();
    for _ in 0..1000 {
        match fs::read_to_string(&*path) {
            Ok(content) => {
                let config: Config = toml::from_str(&content).unwrap();
                assert!(config.account.starts_with("writer-"));
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => panic!("read failed: {err}"),
        }
    }
    for writer in writers {
        writer.join().unwrap();
    }
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    let saved: Config = toml::from_str(&fs::read_to_string(&*path).unwrap()).unwrap();
    assert!(saved.account.starts_with("writer-"));
}
