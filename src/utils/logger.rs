use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use indicatif::{MultiProgress, ProgressBar};

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        $crate::utils::logger::emit(format!("\x1b[36minfo\x1b[0m {}", format!($($arg)*)))
    };
}

#[macro_export]
macro_rules! success {
    ($($arg:tt)*) => {
        $crate::utils::logger::emit(format!("\x1b[32msuccess\x1b[0m {}", format!($($arg)*)))
    };
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        $crate::utils::logger::emit(format!("\x1b[33mwarn\x1b[0m {}", format!($($arg)*)))
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
        $crate::utils::logger::emit(format!("\x1b[34mstep\x1b[0m {}", format!($($arg)*)))
    };
}

#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        if $crate::utils::logger::is_debug() {
            $crate::utils::logger::emit(format!("\x1b[90mdebug\x1b[0m {}", format!($($arg)*)))
        }
    };
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        println!("{}", format!($($arg)*))
    };
}

static VERBOSE_DEBUG: OnceLock<bool> = OnceLock::new();

/// 机器可读模式（`--json`）：stdout 只留给数据，日志一律走 stderr。
///
/// 非 JSON 模式下日志仍写 stdout，保持既有观感；只有明确要「stdout 干净」的
/// 命令才打开这个开关，避免一次改动把所有命令的输出流都换掉。
static JSON_MODE: AtomicBool = AtomicBool::new(false);

pub fn set_json_mode(enabled: bool) {
    JSON_MODE.store(enabled, Ordering::Relaxed);
}

#[must_use]
pub fn is_json_mode() -> bool {
    JSON_MODE.load(Ordering::Relaxed)
}

/// 输出一行日志：JSON 模式走 stderr，否则走 stdout。
pub fn emit(line: String) {
    if is_json_mode() {
        eprintln!("{line}");
    } else {
        println!("{line}");
    }
}

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
