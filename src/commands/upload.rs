use clap::Parser;
use cloud139_core::config::Config;
use std::io::IsTerminal;

#[derive(Parser, Debug, Clone)]
#[command(
    after_help = "单个文件的上传大小上限因账号而异。上传目录时，单个文件失败后仍会处理其余文件。有文件上传失败时，退出码为 1。"
)]
pub struct UploadArgs {
    #[arg(help = "本地文件或目录；目录末尾 / 表示仅上传内容，否则包含目录本身")]
    pub local_path: String,

    #[arg(default_value = "/", help = "远程目录路径")]
    pub remote_path: String,

    #[arg(
        short,
        long,
        help = "允许上传同名文件，由云端为新文件分配名称。文件与目录同名时会报错"
    )]
    pub force: bool,
}

pub(super) async fn execute(args: UploadArgs, config: &Config) -> anyhow::Result<()> {
    let progress = std::io::stderr().is_terminal();
    cloud139_core::services::upload_service::upload_path(
        config,
        &args.local_path,
        &args.remote_path,
        args.force,
        |name, size| {
            if progress {
                crate::presentation::progress::make_progress(name, size)
            } else {
                None
            }
        },
    )
    .await
}
