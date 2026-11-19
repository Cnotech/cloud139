use crate::cli::ListArgs;
use crate::debug;
use crate::presentation::list::{list_json, render_list};
use cloud139::config::Config;
use cloud139::services::list::ListOptions;
use std::fs;

impl<'a> From<&'a ListArgs> for ListOptions<'a> {
    fn from(args: &'a ListArgs) -> Self {
        Self {
            path: &args.path,
            page: args.page,
            page_size: args.page_size,
        }
    }
}

pub(super) async fn execute(args: ListArgs, config: &Config) -> anyhow::Result<()> {
    debug!(
        "准备列出云端目录 path={} page={} page_size={}",
        args.path, args.page, args.page_size
    );
    if args.recursive {
        let results = cloud139::services::list::list_recursive(config, &args.path).await?;
        for result in &results {
            render_list(result);
        }
        if let Some(output) = &args.output {
            let values: Vec<_> = results
                .iter()
                .map(|result| list_json(result, None))
                .collect();
            fs::write(output, serde_json::to_string_pretty(&values)?)?;
        }
        return Ok(());
    }
    let result = cloud139::services::list::list(config, &args).await?;
    render_list(&result);

    if let Some(output) = &args.output {
        let value = list_json(&result, Some((args.page, args.page_size)));
        fs::write(output, serde_json::to_string_pretty(&value)?)?;
    }
    Ok(())
}
