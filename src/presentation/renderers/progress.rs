use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::io::IsTerminal;
use std::time::Duration;

/// 创建下载进度条。
pub fn make_download_progress(remote_path: &str) -> Option<ProgressBar> {
    if !std::io::stderr().is_terminal() {
        return None;
    }
    let pb = ProgressBar::new(0);
    let style = ProgressStyle::with_template(
        "{msg} {bar:24.cyan/blue} {bytes}/{total_bytes} {bytes_per_sec} {eta}",
    )
    .unwrap_or_else(|_| ProgressStyle::default_bar());
    pb.set_style(style);
    pb.set_message(remote_path.to_string());
    Some(pb)
}

/// 创建上传进度条。
pub fn make_upload_progress(
    mp: &MultiProgress,
    file_name: &str,
    file_size: u64,
) -> Option<ProgressBar> {
    if !std::io::stderr().is_terminal() {
        return None;
    }
    let pb = mp.add(ProgressBar::new(file_size));
    let style = ProgressStyle::with_template(
        "{msg} {bar:24.cyan/blue} {bytes}/{total_bytes} {bytes_per_sec} {eta}",
    )
    .unwrap_or_else(|_| ProgressStyle::default_bar());
    pb.set_style(style);
    pb.set_message(file_name.to_string());
    pb.enable_steady_tick(Duration::from_millis(100));
    Some(pb)
}
