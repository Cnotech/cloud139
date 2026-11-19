use std::path::{Path, PathBuf};

const CONFIG_FILE: &str = "cloud139rc.toml";
const GLOBAL_CONFIG_SUBDIR: &str = "cloud139";

fn local_config_path() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(CONFIG_FILE)
}

fn global_config_path() -> PathBuf {
    if let Some(base) = directories::BaseDirs::new() {
        return base
            .home_dir()
            .join(".config")
            .join(GLOBAL_CONFIG_SUBDIR)
            .join(CONFIG_FILE);
    }
    PathBuf::from(CONFIG_FILE)
}

pub(crate) fn config_paths(override_path: Option<&Path>) -> Vec<PathBuf> {
    match override_path {
        Some(path) => vec![path.to_owned()],
        None => vec![local_config_path(), global_config_path()],
    }
}
