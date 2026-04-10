pub use crate::cli::commands::list::ListArgs;
pub use crate::presentation::renderers::list_renderer::format_size;
pub use crate::utils::parse_personal_time;

use crate::presentation::renderers::list_renderer;
use std::fs;

pub async fn execute(args: ListArgs) -> anyhow::Result<()> {
    let config = crate::commands::dispatch::load_config()?;
    let result = crate::application::services::list(&config, &args).await?;
    list_renderer::render_terminal(&result);

    if let Some(output) = &args.output {
        let json = list_renderer::to_json_with_pagination(&result, Some(args.page), Some(args.page_size))?;
        fs::write(output, json)?;
    }
    Ok(())
}