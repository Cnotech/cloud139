use crate::cli::{Cli, Commands};
use crate::config::paths::config_paths;
use anyhow::{Context, Result, ensure};
use cloud139::services::download::resolve_local_path;
use cloud139::utils::remote_path;
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};

pub(crate) fn create(path: &Path, cli: &Cli) -> Result<File> {
    let log_path = resolve_path(path)?;
    let protected = protected_paths(cli);
    for (path, include_children, label) in &protected {
        let path = resolve_path(path)?;
        ensure!(
            !path.starts_with(&log_path) && !(*include_children && log_path.starts_with(&path)),
            "日志文件路径与{label}冲突"
        );
    }
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options
        .open(path)
        .with_context(|| format!("无法新建日志文件 {}", path.display()))?;

    // 新建前无法仅凭路径确定文件系统是否将不同拼写解析为同一名称。
    for (protected, include_children, label) in protected {
        if protected
            .ancestors()
            .any(|candidate| same_file(path, candidate))
            || (include_children
                && log_path
                    .ancestors()
                    .skip(1)
                    .any(|parent| same_file(parent, &protected)))
        {
            drop(file);
            fs::remove_file(path).context("无法清除发生路径冲突的空日志文件")?;
            anyhow::bail!("日志文件路径与{label}冲突");
        }
    }
    Ok(file)
}

#[cfg(unix)]
fn same_file(left: &Path, right: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    match (fs::metadata(left), fs::metadata(right)) {
        (Ok(left), Ok(right)) => (left.dev(), left.ino()) == (right.dev(), right.ino()),
        _ => false,
    }
}

#[cfg(not(unix))]
fn same_file(left: &Path, right: &Path) -> bool {
    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

fn protected_paths(cli: &Cli) -> Vec<(PathBuf, bool, &'static str)> {
    let mut paths: Vec<_> = config_paths(cli.config.as_deref())
        .into_iter()
        .map(|path| (path, false, "配置文件"))
        .collect();
    match &cli.command {
        Commands::Ls(args) => {
            if let Some(output) = &args.output {
                paths.push((PathBuf::from(output), false, "列表输出文件"));
            }
        }
        Commands::Upload(args) => {
            let source = Path::new(&args.local_path);
            paths.push((source.to_owned(), source.is_dir(), "上传源"));
        }
        Commands::Download(args) => {
            if let Ok(remote) = remote_path::normalize(&args.remote_path) {
                let destination = resolve_local_path(&remote, &args.local_path);
                let path = Path::new(&destination);
                paths.push((path.to_owned(), args.recursive || path.is_dir(), "下载目标"));
            }
        }
        Commands::Sync(args) => {
            if !args.src.starts_with("cloud:") {
                let source = Path::new(&args.src);
                paths.push((source.to_owned(), source.is_dir(), "同步源"));
            }
            if !args.dest.starts_with("cloud:") {
                paths.push((PathBuf::from(&args.dest), true, "同步目标"));
            }
        }
        _ => {}
    }

    paths
}

fn resolve_path(path: &Path) -> Result<PathBuf> {
    let absolute = std::path::absolute(path)?;
    if let Ok(resolved) = fs::canonicalize(&absolute) {
        return Ok(resolved);
    }
    // 目标可以尚不存在，先解析已存在的祖先路径。
    for ancestor in absolute.ancestors().skip(1) {
        if let Ok(resolved) = fs::canonicalize(ancestor) {
            return Ok(resolved.join(absolute.strip_prefix(ancestor)?));
        }
    }
    Ok(absolute)
}
