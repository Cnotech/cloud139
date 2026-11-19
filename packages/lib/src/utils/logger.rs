// 导出宏通过本库访问日志级别，调用方无需直接依赖 log。
#[doc(hidden)]
pub use log::Level;

#[doc(hidden)]
pub fn emit(level: Level, label: &str, message: std::fmt::Arguments<'_>) {
    log::log!(target: label, level, "{message}");
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        $crate::utils::logger::emit($crate::utils::logger::Level::Info, "cloud139::info", format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! success {
    ($($arg:tt)*) => {
        $crate::utils::logger::emit($crate::utils::logger::Level::Info, "cloud139::success", format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        $crate::utils::logger::emit($crate::utils::logger::Level::Warn, "cloud139::warn", format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        $crate::utils::logger::emit($crate::utils::logger::Level::Error, "cloud139::error", format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! step {
    ($($arg:tt)*) => {
        $crate::utils::logger::emit($crate::utils::logger::Level::Info, "cloud139::step", format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        $crate::utils::logger::emit($crate::utils::logger::Level::Debug, "cloud139::debug", format_args!($($arg)*))
    };
}
