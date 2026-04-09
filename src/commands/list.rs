pub use crate::cli::commands::list::ListArgs;
use crate::presentation::renderers::list_renderer;
use anyhow::Context;
use std::fs;

pub async fn execute(args: ListArgs) -> anyhow::Result<()> {
    let config = crate::config::Config::load().context("加载配置失败")?;
    let result = crate::application::services::list(&config, &args).await?;
    list_renderer::render_terminal(&result);

    if let Some(output) = &args.output {
        let json = list_renderer::to_json(&result)?;
        fs::write(output, json)?;
    }
    Ok(())
}