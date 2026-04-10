pub use crate::cli::commands::list::ListArgs;
pub use crate::presentation::renderers::list_renderer::format_size;

pub fn parse_personal_time(time_str: &str) -> String {
    if time_str.is_empty() {
        return String::new();
    }
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(time_str) {
        return dt.format("%Y-%m-%d %H:%M:%S").to_string();
    }
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(time_str, "%Y-%m-%dT%H:%M:%S%.f") {
        return dt.format("%Y-%m-%d %H:%M:%S").to_string();
    }
    time_str.to_string()
}

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