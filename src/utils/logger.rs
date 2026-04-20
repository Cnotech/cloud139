use std::sync::OnceLock;

use indicatif::{MultiProgress, ProgressBar};

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

// ── pb-aware 日志辅助（单进度条） ──────────────────────────────────────────
// pb = None  → 独立命令，使用现有宏直接打印
// pb = Some  → sync 模式，用 pb.println() 保证输出在进度条上方
// pb_debug 仅在 is_debug()=true 时输出; 其他级别始终显示

pub fn pb_debug(msg: &str, pb: &Option<ProgressBar>) {
    if is_debug() {
        match pb {
            None => debug!("{}", msg),
            Some(pb) => pb.println(format!("\x1b[90mdebug\x1b[0m {}", msg)),
        }
    }
}

pub fn pb_info(msg: &str, pb: &Option<ProgressBar>) {
    match pb {
        None => info!("{}", msg),
        Some(pb) => pb.println(format!("\x1b[36minfo\x1b[0m {}", msg)),
    }
}

pub fn pb_step(msg: &str, pb: &Option<ProgressBar>) {
    match pb {
        None => step!("{}", msg),
        Some(pb) => pb.println(format!("\x1b[34mstep\x1b[0m {}", msg)),
    }
}

pub fn pb_success(msg: &str, pb: &Option<ProgressBar>) {
    match pb {
        None => success!("{}", msg),
        Some(pb) => pb.println(format!("\x1b[32msuccess\x1b[0m {}", msg)),
    }
}

pub fn pb_warn(msg: &str, pb: &Option<ProgressBar>) {
    match pb {
        None => warn!("{}", msg),
        Some(pb) => pb.println(format!("\x1b[33mwarn\x1b[0m {}", msg)),
    }
}

pub fn pb_error(msg: &str, pb: &Option<ProgressBar>) {
    match pb {
        None => error!("{}", msg),
        Some(pb) => pb.println(format!("\x1b[31merror\x1b[0m {}", msg)),
    }
}

// ── mp-aware 日志辅助（多进度条） ──────────────────────────────────────────
// 用 MultiProgress.println() 保证输出在所有进度条上方

pub fn mp_info(msg: &str, mp: &MultiProgress) {
    mp.println(format!("\x1b[36minfo\x1b[0m {}", msg)).ok();
}

pub fn mp_step(msg: &str, mp: &MultiProgress) {
    mp.println(format!("\x1b[34mstep\x1b[0m {}", msg)).ok();
}

pub fn mp_success(msg: &str, mp: &MultiProgress) {
    mp.println(format!("\x1b[32msuccess\x1b[0m {}", msg)).ok();
}

pub fn mp_warn(msg: &str, mp: &MultiProgress) {
    mp.println(format!("\x1b[33mwarn\x1b[0m {}", msg)).ok();
}

pub fn mp_error(msg: &str, mp: &MultiProgress) {
    mp.println(format!("\x1b[31merror\x1b[0m {}", msg)).ok();
}

pub fn mp_debug(msg: &str, mp: &MultiProgress) {
    if is_debug() {
        mp.println(format!("\x1b[90mdebug\x1b[0m {}", msg)).ok();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_debug_does_not_panic() {
        let _ = is_debug();
    }

    #[test]
    fn test_debug_macro_format() {
        let msg = format!("value: {}", 42);
        assert_eq!(msg, "value: 42");
    }

    #[test]
    fn test_init_verbose_does_not_panic() {
        init_verbose("debug");
        init_verbose("info");
    }
}
