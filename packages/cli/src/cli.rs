use clap::{Parser, Subcommand};
use cloud139::services::sync::executor::MAX_JOBS;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "cloud139")]
#[command(about = "中国移动云盘命令行工具", long_about = None)]
#[command(version)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Commands,

    #[arg(short, long, default_value = "info")]
    pub(crate) verbose: String,

    /// 指定配置文件路径 (覆盖默认查找顺序)
    #[arg(short, long, global = true, value_name = "PATH")]
    pub(crate) config: Option<PathBuf>,

    /// 新建文件并写入调试日志
    #[arg(long, global = true, value_name = "PATH")]
    pub(crate) log_file: Option<PathBuf>,
}

#[derive(Subcommand)]
pub(crate) enum Commands {
    /// 登录账号
    Login(LoginArgs),
    /// 列出文件和目录
    Ls(ListArgs),
    /// 上传文件和目录
    Upload(UploadArgs),
    /// 下载文件和目录
    Download(DownloadArgs),
    /// 删除文件和目录
    Rm(DeleteArgs),
    /// 创建目录
    Mkdir(MkdirArgs),
    /// 移动文件和目录
    Mv(MvArgs),
    /// 复制文件和目录
    Cp(CpArgs),
    /// 重命名文件和目录
    Rename(RenameArgs),
    /// 同步本地目录和云端目录
    Sync(SyncArgs),
}

#[derive(Parser, Debug, Clone)]
pub(crate) struct CpArgs {
    #[arg(required = true, help = "源文件或目录路径（支持多个，用空格分隔）")]
    pub(crate) source: Vec<String>,

    #[arg(help = "目标目录")]
    pub(crate) target: String,

    #[arg(
        short,
        long,
        help = "目标目录已有同名文件时，仍提交复制。涉及目录的同名冲突会报错"
    )]
    pub(crate) force: bool,
}

#[derive(Parser, Debug)]
pub(crate) struct DeleteArgs {
    #[arg(short = 'r', long, help = "确认递归删除目录（仍需 --yes）")]
    pub(crate) recursive: bool,

    #[arg(help = "远程文件或目录路径")]
    pub(crate) path: String,

    #[arg(short, long, help = "确认删除")]
    pub(crate) yes: bool,
}

#[derive(Parser, Debug)]
pub(crate) struct DownloadArgs {
    #[arg(short = 'r', long, help = "递归下载目录，保留空目录")]
    pub(crate) recursive: bool,

    #[arg(help = "远程文件或目录路径")]
    pub(crate) remote_path: String,

    #[arg(help = "本地保存路径（默认保存到当前目录的同名文件）")]
    pub(crate) local_path: Option<String>,
}

#[derive(Parser, Debug, Clone)]
pub(crate) struct ListArgs {
    #[arg(short = 'R', long, help = "递归列出所有目录及全部分页")]
    pub(crate) recursive: bool,

    #[arg(default_value = "/", help = "远程目录路径")]
    pub(crate) path: String,

    #[arg(short, long, default_value = "1", help = "页码")]
    pub(crate) page: i32,

    #[arg(short = 's', long, default_value = "100", help = "每页数量")]
    pub(crate) page_size: i32,

    #[arg(short, long, help = "将JSON输出到指定文件")]
    pub(crate) output: Option<String>,
}

#[derive(Parser, Debug, Clone)]
pub(crate) struct LoginArgs {
    #[arg(
        short,
        long,
        required = true,
        help = "Authorization Token (从浏览器开发者工具获取)"
    )]
    pub(crate) token: String,
}

#[derive(Parser, Debug, Clone)]
pub(crate) struct MkdirArgs {
    #[arg(short = 'p', long, help = "递归创建父目录；已有目录成功，同名文件报错")]
    pub(crate) parents: bool,

    #[arg(help = "新目录路径，格式: /父目录/新目录名")]
    pub(crate) path: String,
}

#[derive(Parser, Debug, Clone)]
pub(crate) struct MvArgs {
    #[arg(required = true, help = "源文件或目录路径（支持多个，用空格分隔）")]
    pub(crate) source: Vec<String>,

    #[arg(help = "必须是已有目标目录")]
    pub(crate) target: String,

    #[arg(
        short,
        long,
        help = "目标目录已有同名文件时，仍提交移动。涉及目录的同名冲突会报错"
    )]
    pub(crate) force: bool,
}

#[derive(Parser, Debug, Clone)]
pub(crate) struct RenameArgs {
    #[arg(help = "源文件或目录路径")]
    pub(crate) source: String,

    #[arg(help = "新名称")]
    pub(crate) target: String,

    #[arg(
        short,
        long,
        help = "新名称已被其他文件使用时，仍提交重命名。涉及目录的同名冲突会报错"
    )]
    pub(crate) force: bool,
}

fn parse_jobs(value: &str) -> Result<usize, String> {
    let jobs = value
        .parse::<usize>()
        .map_err(|_| "并发数量必须为正整数".to_string())?;
    if !(1..=MAX_JOBS).contains(&jobs) {
        return Err(format!("并发数量必须介于 1 和 {MAX_JOBS} 之间"));
    }
    Ok(jobs)
}

#[derive(Parser, Debug, Clone)]
pub(crate) struct SyncArgs {
    #[arg(help = "源路径，本地路径或 cloud:/remote/path")]
    pub(crate) src: String,

    #[arg(help = "目标路径，本地路径或 cloud:/remote/path")]
    pub(crate) dest: String,

    #[arg(short = 'r', long, help = "递归同步子目录，空目录也会同步")]
    pub(crate) recursive: bool,

    #[arg(short = 'n', long, help = "只预览同步计划")]
    pub(crate) dry_run: bool,

    #[arg(long, help = "删除目标中源没有的文件或空目录")]
    pub(crate) delete: bool,

    #[arg(long, value_name = "PAT", help = "排除匹配的路径，可多次指定")]
    pub(crate) exclude: Vec<String>,

    #[arg(short = 'j', long, default_value_t = 4, default_value_if("verify", "true", Some("1")),
        value_parser = parse_jobs, help = format!("同时传输的文件数，范围 1 到 {MAX_JOBS}。默认 4，--verify 固定为 1"))]
    pub(crate) jobs: usize,

    #[arg(long, help = "校验内容并修复不一致的文件")]
    pub(crate) verify: bool,

    #[arg(long, help = "以 JSON 报告本次同步的文件变更")]
    pub(crate) json: bool,
}

#[derive(Parser, Debug, Clone)]
#[command(
    after_help = "单个文件的上传大小上限因账号而异。上传目录时，单个文件失败后仍会处理其余文件。有文件上传失败时，退出码为 1。"
)]
pub(crate) struct UploadArgs {
    #[arg(help = "本地文件或目录；目录末尾 / 表示仅上传内容，否则包含目录本身")]
    pub(crate) local_path: String,

    #[arg(default_value = "/", help = "远程目录路径")]
    pub(crate) remote_path: String,

    #[arg(
        short,
        long,
        help = "允许上传同名文件，由云端为新文件分配名称。文件与目录同名时会报错"
    )]
    pub(crate) force: bool,
}
