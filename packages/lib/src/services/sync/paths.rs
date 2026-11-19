use crate::models::sync_item::{SyncDirection, SyncEndpoint};
use crate::utils::remote_path;
use anyhow::{Result, anyhow};
use glob::Pattern;
use std::path::PathBuf;

pub fn parse_sync_endpoint(value: &str) -> SyncEndpoint {
    if let Some(rest) = value.strip_prefix("cloud:") {
        return SyncEndpoint::Cloud(remote_path::normalize_cloud_path(rest));
    }
    SyncEndpoint::Local(PathBuf::from(value))
}

pub(super) fn normalize_sync_endpoint(endpoint: SyncEndpoint) -> Result<SyncEndpoint> {
    match endpoint {
        SyncEndpoint::Cloud(path) => Ok(SyncEndpoint::Cloud(remote_path::normalize(&path)?)),
        local => Ok(local),
    }
}

pub fn sync_direction(src: &SyncEndpoint, dest: &SyncEndpoint) -> Result<SyncDirection> {
    match (src, dest) {
        (SyncEndpoint::Local(_), SyncEndpoint::Cloud(_)) => Ok(SyncDirection::LocalToCloud),
        (SyncEndpoint::Cloud(_), SyncEndpoint::Local(_)) => Ok(SyncDirection::CloudToLocal),
        (SyncEndpoint::Local(_), SyncEndpoint::Local(_)) => {
            Err(anyhow!("SRC 和 DEST 都是本地路径，请使用 cp/mv 或系统工具"))
        }
        (SyncEndpoint::Cloud(_), SyncEndpoint::Cloud(_)) => {
            Err(anyhow!("SRC 和 DEST 都是云端路径，请使用 cp/mv 命令"))
        }
    }
}

pub(super) fn compile_exclude_patterns(exclude: &[String]) -> Result<Vec<Pattern>> {
    exclude
        .iter()
        .map(|pattern| {
            Pattern::new(pattern).map_err(|err| anyhow!("无效 exclude glob `{}`: {}", pattern, err))
        })
        .collect()
}

pub(in crate::services) fn is_excluded(rel_path: &str, patterns: &[Pattern]) -> bool {
    let options = glob::MatchOptions {
        case_sensitive: true,
        require_literal_separator: true,
        require_literal_leading_dot: false,
    };
    patterns.iter().any(|pattern| {
        // 直接匹配路径。
        if pattern.matches_with(rel_path, options) {
            return true;
        }
        // 像 `dir/**` 这样的规则也应匹配 `dir` 目录本身，
        // 这样 `--exclude target/**` 会一并排除 `target` 目录。
        let pattern_str = pattern.as_str();
        if let Some(prefix) = pattern_str.strip_suffix("/**")
            && rel_path == prefix
        {
            return true;
        }
        false
    })
}

pub(in crate::services) fn path_and_ancestors(path: &str) -> impl Iterator<Item = &str> {
    std::iter::successors(Some(path), |path| {
        path.rsplit_once('/').map(|(parent, _)| parent)
    })
}

pub(in crate::services) const SYNC_UPLOAD_PREFIX: &str = ".cloud139-upload-";
pub(in crate::services) const UPLOAD_BACKUP_PREFIX: &str = ".cloud139-backup-";

pub(super) fn is_recovery_name(name: &str) -> bool {
    name.starts_with(SYNC_UPLOAD_PREFIX) || name.starts_with(UPLOAD_BACKUP_PREFIX)
}

pub(super) fn is_recovery_path(path: &str) -> bool {
    path.split('/').any(is_recovery_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_excluded_double_star_matches_nested_path() {
        let patterns = compile_exclude_patterns(&["target/**".to_string()]).unwrap();
        assert!(is_excluded("target/debug/app", &patterns));
        assert!(is_excluded("target/build", &patterns));
        assert!(!is_excluded("src/main.rs", &patterns));
    }

    #[test]
    fn test_is_excluded_single_star_does_not_cross_dirs() {
        let patterns = compile_exclude_patterns(&["*.log".to_string()]).unwrap();
        assert!(is_excluded("app.log", &patterns));
        assert!(is_excluded("debug.log", &patterns));
        assert!(!is_excluded("dir/app.log", &patterns));
    }
}
