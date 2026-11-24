use cloud139::progress::{Progress, TransferProgress};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

static PROGRESS: OnceLock<MultiProgress> = OnceLock::new();

pub(crate) fn suspend(output: impl FnOnce()) {
    match PROGRESS.get() {
        Some(progress) => progress.suspend(output),
        None => output(),
    }
}

pub(crate) fn make_progress(name: &str, bytes: u64) -> Option<Progress> {
    let progress = PROGRESS.get_or_init(MultiProgress::new);
    let bar = progress.add(ProgressBar::new(bytes));
    let style = ProgressStyle::with_template(
        "{msg} {bar:24.cyan/blue} {bytes}/{total_bytes} {bytes_per_sec} {eta}",
    )
    .unwrap_or_else(|_| ProgressStyle::default_bar());
    bar.set_style(style);
    bar.set_message(name.to_owned());
    bar.enable_steady_tick(Duration::from_millis(100));
    Some(Arc::new(TerminalProgress(bar)))
}

struct TerminalProgress(ProgressBar);

impl TransferProgress for TerminalProgress {
    fn reset(&self) {
        self.0.set_position(0);
        self.0.reset_eta();
    }

    fn set_length(&self, bytes: u64) {
        self.0.set_length(bytes);
    }

    fn inc(&self, bytes: u64) {
        self.0.inc(bytes);
    }

    fn finish(&self) {
        self.0.finish_and_clear();
    }
}

#[cfg(test)]
mod tests {
    use super::{TerminalProgress, make_progress, suspend};
    use cloud139::progress::TransferProgress;
    use indicatif::{ProgressBar, ProgressDrawTarget};

    #[test]
    fn transfer_progress_can_reset_after_a_partial_attempt() {
        let bar = ProgressBar::with_draw_target(Some(10), ProgressDrawTarget::hidden());
        let progress = TerminalProgress(bar.clone());
        progress.inc(4);
        assert_eq!(bar.position(), 4);

        progress.reset();
        assert_eq!(bar.position(), 0);
        progress.set_length(5);
        assert_eq!(bar.length(), Some(5));
        progress.inc(5);
        progress.finish();
        assert!(bar.is_finished());
    }

    #[test]
    fn active_progress_allows_status_output() {
        let progress = make_progress("note.txt", 5).unwrap();
        let mut calls = 0;
        suspend(|| calls += 1);
        progress.finish();
        assert_eq!(calls, 1);
    }
}
