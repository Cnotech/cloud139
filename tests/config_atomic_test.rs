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
    // 原子写入的临时文件在完成后已清理（持久锁文件保留）。
    assert!(fs::read_dir(dir.path()).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".cloud139-config-")
    }));
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

// 独立测试进程持锁，父测试终止它以模拟写入进程崩溃。
#[test]
fn lock_holder_child() {
    let Some(path) = std::env::var_os("CLOUD139_TEST_LOCK_PATH") else {
        return;
    };
    let path = std::path::PathBuf::from(path);
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path.with_extension("toml.lock"))
        .unwrap();
    fs2::FileExt::lock_exclusive(&lock).unwrap();
    fs::write(path.with_extension("ready"), "ready").unwrap();
    std::thread::sleep(std::time::Duration::from_secs(30));
}

#[test]
fn config_lock_is_cross_process_and_released_after_crash() {
    use cloud139::config::ConfigError;
    use std::time::{Duration, Instant};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    config("first").save_to(&path).unwrap();
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "lock_holder_child"])
        .env("CLOUD139_TEST_LOCK_PATH", &path)
        .stdout(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let start = Instant::now();
    while !path.with_extension("ready").exists() && start.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(10));
    }
    let ready = path.with_extension("ready").exists();
    let result = config("second").save_to(&path);
    // 无论断言结果如何先回收进程，避免测试失败时留下持锁子进程。
    child.kill().unwrap();
    child.wait().unwrap();
    assert!(ready, "child did not acquire the config lock");
    assert!(matches!(result, Err(ConfigError::Busy)));
    let previous: Config = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(previous.account, "first");
    config("second").save_to(&path).unwrap();
}
