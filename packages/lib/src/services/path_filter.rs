//! 本地和云端扫描共用的路径排除规则。

use anyhow::{Result, anyhow};
use glob::Pattern;

pub(super) fn compile_exclude_patterns(exclude: &[String]) -> Result<Vec<Pattern>> {
    exclude
        .iter()
        .map(|pattern| {
            Pattern::new(pattern).map_err(|err| anyhow!("无效 exclude glob `{}`: {}", pattern, err))
        })
        .collect()
}

pub(super) fn is_excluded(rel_path: &str, patterns: &[Pattern]) -> bool {
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

pub(super) fn path_and_ancestors(path: &str) -> impl Iterator<Item = &str> {
    std::iter::successors(Some(path), |path| {
        path.rsplit_once('/').map(|(parent, _)| parent)
    })
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
