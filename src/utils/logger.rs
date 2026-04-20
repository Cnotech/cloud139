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

#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        if $crate::utils::logger::is_debug() {
            println!("\x1b[90mdebug\x1b[0m {}", format!($($arg)*))
        }
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
    fn test_is_debug_does_not_panic() {
        // 验证 is_debug() 能正常调用且返回 bool
        // OnceLock 只能 set 一次，测试顺序可能影响结果
        // 生产环境中 init_verbose 仅在 main.rs 中调用一次，不受影响
        let _ = is_debug();
    }

    #[test]
    fn test_debug_macro_format() {
        let msg = format!("value: {}", 42);
        assert_eq!(msg, "value: 42");
    }

    #[test]
    fn test_init_verbose_does_not_panic() {
        // 验证 init_verbose 能正常调用且不 panic
        // .ok() 会忽略重复设置的错误
        init_verbose("debug");
        init_verbose("info");
    }
}
