use clap::Parser;
use crate::debug;
use crate::presentation::list_renderer;
use std::fs;

#[derive(Parser, Debug, Clone)]
pub struct ListArgs {
    #[arg(default_value = "/", help = "远程目录路径")]
    pub path: String,

    #[arg(short, long, default_value = "1", help = "页码")]
    pub page: i32,

    #[arg(short = 's', long, default_value = "100", help = "每页数量")]
    pub page_size: i32,

    #[arg(short, long, help = "将JSON输出到指定文件")]
    pub output: Option<String>,
}

pub async fn execute(args: ListArgs) -> anyhow::Result<()> {
    let config = crate::config::Config::load()?;
    debug!(
        "list: path={}, page={}, page_size={}",
        args.path, args.page, args.page_size
    );
    let result = crate::services::list(&config, &args).await?;
    list_renderer::render_terminal(&result);

    if let Some(output) = &args.output {
        let json =
            list_renderer::to_json_with_pagination(&result, Some(args.page), Some(args.page_size))?;
        fs::write(output, json)?;
    }
    Ok(())
}
