use crate::config::Config;
use crate::debug;
use crate::presentation::list_renderer;
use clap::Parser;
use std::fs;

#[derive(Parser, Debug, Clone)]
pub struct ListArgs {
    #[arg(short = 'R', long, help = "递归列出所有目录及全部分页")]
    pub recursive: bool,

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
    execute_with_config(args, &Config::load_personal()?).await
}

pub(super) async fn execute_with_config(args: ListArgs, config: &Config) -> anyhow::Result<()> {
    debug!(
        "准备列出云端目录 path={} page={} page_size={}",
        args.path, args.page, args.page_size
    );
    if args.recursive {
        let results = crate::services::list_service::list_recursive(config, &args.path).await?;
        for result in &results {
            list_renderer::render_terminal(result);
        }
        if let Some(output) = &args.output {
            let values: Vec<_> = results
                .iter()
                .map(|result| list_renderer::json_output(result, None))
                .collect();
            fs::write(output, serde_json::to_string_pretty(&values)?)?;
        }
        return Ok(());
    }
    let result = crate::services::list(config, &args).await?;
    list_renderer::render_terminal(&result);

    if let Some(output) = &args.output {
        let value = list_renderer::json_output(&result, Some((args.page, args.page_size)));
        fs::write(output, serde_json::to_string_pretty(&value)?)?;
    }
    Ok(())
}
