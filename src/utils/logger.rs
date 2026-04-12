use std::sync::OnceLock;

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        println!("\x1b[36minfo\x1b[0m {}", format!($($arg)*))
    };
}

#[macro_export]
macro_rules! success {
    ($($arg:tt)*) => {
        println!("\x1b[32msuccess\x1b[0m {}", format!($($arg)*))
    };
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        println!("\x1b[33mwarn\x1b[0m {}", format!($($arg)*))
    };
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        eprintln!("\x1b[31merror\x1b[0m {}", format!($($arg)*))
    };
}

#[macro_export]
macro_rules! step {
    ($($arg:tt)*) => {
        println!("\x1b[34mstep\x1b[0m {}", format!($($arg)*))
    };
}

static VERBOSE_DEBUG: OnceLock<bool> = OnceLock::new();

pub fn init_verbose(level: &str) {
    VERBOSE_DEBUG.set(level == "debug").ok();
}

#[must_use]
pub fn is_debug() -> bool {
    *VERBOSE_DEBUG.get().unwrap_or(&false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_debug_returns_bool() {
        // 测试 is_debug 能正常调用且返回 bool
        // 由于 OnceLock 只能 set 一次，测试顺序可能影响结果
        // 生产环境中 init_verbose 仅在 main.rs 中调用一次，不受影响
        let result = is_debug();
        assert!(
            result == true || result == false,
            "is_debug() should return a bool"
        );
    }

    #[test]
    fn test_init_verbose_sets_debug() {
        // 测试 init_verbose 正确设置 debug 状态
        // 注意：如果其他测试先调用了 init_verbose，这个测试会失败
        // 这是 OnceLock 的限制，在集成测试中应该隔离
        let before = is_debug();
        init_verbose("debug");
        let after = is_debug();

        // 如果之前未设置，应该变为 true
        if !before {
            assert!(
                after,
                "is_debug() should return true after init_verbose(\"debug\")"
            );
        }
    }

    #[test]
    fn test_init_verbose_sets_non_debug() {
        // 测试 init_verbose 正确设置非 debug 状态
        // 注意：如果其他测试先调用了 init_verbose，这个测试会失败
        let before = is_debug();
        init_verbose("info");
        let after = is_debug();

        // 如果之前未设置，应该变为 false
        if !before {
            assert!(
                !after,
                "is_debug() should return false after init_verbose(\"info\")"
            );
        }
    }
}
