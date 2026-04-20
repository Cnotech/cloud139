use crate::{debug, step, success};
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use std::io::IsTerminal;
use std::path::Path;

#[derive(Parser, Debug)]
pub struct DownloadArgs {
    #[arg(help = "远程文件路径")]
    pub remote_path: String,

    #[arg(help = "本地保存路径（默认保存到当前目录的同名文件）")]
    pub local_path: Option<String>,
}

pub fn resolve_local_path(remote_path: &str, local_path: &Option<String>) -> String {
    match local_path {
        Some(path) if !path.is_empty() => {
            let ends_with_slash = path.ends_with('/');
            let path = path.trim_end_matches('/');
            let path_obj = Path::new(path);
            if path_obj.is_dir() || ends_with_slash {
                let parts: Vec<&str> = remote_path.trim_start_matches('/').rsplit('/').collect();
                let file_name = parts.first().copied().unwrap_or(remote_path);
                if file_name.is_empty() || file_name == remote_path {
                    format!("{}/download", path)
                } else {
                    format!("{}/{}", path, file_name)
                }
            } else {
                path.to_string()
            }
        }
        _ => {
            let parts: Vec<&str> = remote_path.trim_start_matches('/').rsplit('/').collect();
            let file_name = parts.first().copied().unwrap_or(remote_path);
            if file_name.is_empty() || file_name == remote_path {
                "download".to_string()
            } else {
                file_name.to_string()
            }
        }
    }
}

fn make_download_progress(remote_path: &str) -> Option<ProgressBar> {
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

pub async fn execute(args: DownloadArgs) -> anyhow::Result<()> {
    let config = crate::commands::dispatch::load_config()?;

    let remote_path = &args.remote_path;
    let local_path = resolve_local_path(remote_path, &args.local_path);

    debug!("下载链接: {}", remote_path);
    step!("开始下载到: {:?}", local_path);

    let pb = make_download_progress(remote_path);
    crate::application::services::download(&config, remote_path, &local_path, pb).await?;

    success!("下载完成!");
    Ok(())
}
