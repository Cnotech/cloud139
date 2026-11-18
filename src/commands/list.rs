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
    let config = crate::config::Config::load_personal()?;
    debug!(
        "list: path={}, page={}, page_size={}",
        args.path, args.page, args.page_size
    );
    if args.recursive {
        let results = crate::services::list_service::list_recursive(&config, &args.path).await?;
        for result in &results {
            list_renderer::render_terminal(result);
        }
        if let Some(output) = &args.output {
            let values = results
                .iter()
                .map(|result| {
                    list_renderer::to_json_with_pagination(result, None, None).and_then(|json| {
                        serde_json::from_str::<serde_json::Value>(&json).map_err(Into::into)
                    })
                })
                .collect::<anyhow::Result<Vec<_>>>()?;
            fs::write(output, serde_json::to_string_pretty(&values)?)?;
        }
        return Ok(());
    }
    let result = crate::services::list(&config, &args).await?;
    list_renderer::render_terminal(&result);

    if let Some(output) = &args.output {
        let json =
            list_renderer::to_json_with_pagination(&result, Some(args.page), Some(args.page_size))?;
        fs::write(output, json)?;
    }
    Ok(())
}
