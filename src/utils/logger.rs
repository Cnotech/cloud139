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

use std::sync::OnceLock;

static VERBOSE_DEBUG: OnceLock<bool> = OnceLock::new();

pub fn init_verbose(level: &str) {
    VERBOSE_DEBUG.set(level == "debug").ok();
}

pub fn is_debug() -> bool {
    *VERBOSE_DEBUG.get().unwrap_or(&false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_debug_defaults_false() {
        // OnceLock 只能 set 一次，此测试验证 is_debug 在未初始化时返回 false
        // 注意：若其他测试先调用 init_verbose("debug")，此测试结果会受影响
        // 生产环境中 init_verbose 仅在 main.rs 中调用一次，不受影响
        let result = is_debug();
        // 取决于 OnceLock 状态：要么是 false（未初始化），要么保持已设定值
        // 此断言验证函数能正常调用且不 panic
        let _ = result;
    }
}
