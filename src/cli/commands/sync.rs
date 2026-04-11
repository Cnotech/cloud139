use crate::commands::sync::SyncArgs as CommandSyncArgs;
use clap::Parser;

fn parse_jobs(value: &str) -> Result<usize, String> {
    let jobs = value
        .parse::<usize>()
        .map_err(|_| "jobs must be a positive integer".to_string())?;
    if jobs == 0 {
        return Err("jobs must be greater than 0".to_string());
    }
    Ok(jobs)
}

#[derive(Parser, Debug, Clone)]
pub struct SyncArgs {
    #[arg(help = "源路径，本地路径或 cloud:/remote/path")]
    pub src: String,

    #[arg(help = "目标路径，本地路径或 cloud:/remote/path")]
    pub dest: String,

    #[arg(short = 'r', long, help = "递归同步子目录")]
    pub recursive: bool,

    #[arg(short = 'n', long, help = "演习模式，只输出操作计划")]
    pub dry_run: bool,

    #[arg(long, help = "删除目标中源没有的文件")]
    pub delete: bool,

    #[arg(long, help = "用 SHA-1 校验和替代大小和修改时间做对比")]
    pub checksum: bool,

    #[arg(long, value_name = "PAT", help = "排除匹配的路径，可多次指定")]
    pub exclude: Vec<String>,

    #[arg(short = 'j', long, default_value = "4", value_parser = parse_jobs, help = "并发传输数量上限")]
    pub jobs: usize,
}

impl From<SyncArgs> for CommandSyncArgs {
    fn from(args: SyncArgs) -> Self {
        CommandSyncArgs {
            src: args.src,
            dest: args.dest,
            recursive: args.recursive,
            dry_run: args.dry_run,
            delete: args.delete,
            checksum: args.checksum,
            exclude: args.exclude,
            jobs: args.jobs,
        }
    }
}
