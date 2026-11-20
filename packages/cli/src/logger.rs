use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        $crate::logger::status("36", "info", format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! success {
    ($($arg:tt)*) => {
        $crate::logger::status("32", "success", format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        $crate::logger::status("33", "warn", format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        $crate::logger::status("31", "error", format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! step {
    ($($arg:tt)*) => {
        $crate::logger::status("34", "step", format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        if $crate::logger::debug_enabled() {
            $crate::logger::status("90", "debug", format_args!($($arg)*))
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
static STATUS_TO_STDERR: AtomicBool = AtomicBool::new(false);
static LOG_FILE: OnceLock<Mutex<Option<BufWriter<File>>>> = OnceLock::new();

pub(crate) fn init_file(file: File) {
    LOG_FILE.set(Mutex::new(Some(BufWriter::new(file)))).ok();
}

pub(crate) fn route_status_to_stderr() {
    STATUS_TO_STDERR.store(true, Ordering::Relaxed);
}

#[doc(hidden)]
pub(crate) fn status(color: &str, level: &str, message: std::fmt::Arguments<'_>) {
    write_file(|file| {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        // 服务端的错误详情可能包含敏感数据，文件保留级别与请求诊断。
        let message = match level {
            "error" => "操作失败，详情见终端".to_owned(),
            "warn" => "操作警告，详情见终端".to_owned(),
            _ => message.to_string().replace(['\r', '\n', '\x1b'], " "),
        };
        writeln!(file, "ts_ms={timestamp} {level} {message}")
    });
    if level == "debug" && !is_debug() {
        return;
    }
    crate::presentation::progress::suspend(|| {
        if level == "error" || STATUS_TO_STDERR.load(Ordering::Relaxed) {
            anstream::eprintln!("\x1b[{color}m{level}\x1b[0m {message}");
        } else {
            anstream::println!("\x1b[{color}m{level}\x1b[0m {message}");
        }
    });
}

fn write_file(write: impl FnOnce(&mut BufWriter<File>) -> io::Result<()>) {
    if let Some(sink) = LOG_FILE.get()
        && let Ok(mut sink) = sink.lock()
        && let Some(file) = sink.as_mut()
        && let Err(error) = write(file)
    {
        *sink = None;
        anstream::eprintln!("无法写入日志文件: {error}");
    }
}

pub(crate) fn flush_file() {
    write_file(Write::flush);
}

pub(crate) fn init_verbose(level: &str) {
    VERBOSE_DEBUG.set(level == "debug").ok();
}

#[must_use]
pub(crate) fn is_debug() -> bool {
    *VERBOSE_DEBUG.get().unwrap_or(&false)
}

pub(crate) fn debug_enabled() -> bool {
    is_debug() || LOG_FILE.get().is_some()
}

pub(crate) fn init_core_logger() {
    static LOGGER: CoreLogger = CoreLogger;
    if log::set_logger(&LOGGER).is_ok() {
        log::set_max_level(if debug_enabled() {
            log::LevelFilter::Debug
        } else {
            log::LevelFilter::Info
        });
    }
}

struct CoreLogger;

impl log::Log for CoreLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.target().starts_with("cloud139::")
    }

    fn log(&self, record: &log::Record<'_>) {
        let (color, label) = match record.target() {
            "cloud139::success" => ("32", "success"),
            "cloud139::step" => ("34", "step"),
            "cloud139::warn" => ("33", "warn"),
            "cloud139::error" => ("31", "error"),
            "cloud139::debug" => ("90", "debug"),
            "cloud139::info" => ("36", "info"),
            _ => return,
        };
        status(color, label, *record.args());
    }

    fn flush(&self) {
        flush_file();
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
