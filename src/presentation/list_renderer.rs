use crate::application::services::list_service::ListResult;
use crate::domain::file_item::EntryKind;
use crate::print;
use serde::Serialize;

pub fn format_size(size: i64) -> String {
    if size < 1024 {
        format!("{} B", size)
    } else if size < 1024 * 1024 {
        format!("{:.2} KB", size as f64 / 1024.0)
    } else if size < 1024 * 1024 * 1024 {
        format!("{:.2} MB", size as f64 / 1024.0 / 1024.0)
    } else {
        format!("{:.2} GB", size as f64 / 1024.0 / 1024.0 / 1024.0)
    }
}

pub fn render_terminal(result: &ListResult) {
    print!("\n文件列表 ({}):", result.path);
    print!("{:<40} {:>15} {:<20}", "名称", "大小", "修改时间");
    print!("{}", "-".repeat(80));

    for item in &result.items {
        let marker = match item.kind {
            EntryKind::Folder => "d",
            EntryKind::File => "-",
        };

        print!(
            "{} {:<38} {:>15} {:<20}",
            marker,
            item.name,
            format_size(item.size),
            item.modified
        );
    }
}

#[derive(Serialize)]
pub struct JsonListOutput {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<i32>,
    pub total: i32,
    pub items: Vec<JsonFileItem>,
}

#[derive(Serialize)]
pub struct JsonFileItem {
    pub name: String,
    #[serde(rename = "type")]
    pub file_type: String,
    pub size: i64,
    pub modified: String,
}

pub fn to_json(result: &ListResult) -> anyhow::Result<String> {
    to_json_with_pagination(result, None, None)
}

pub fn to_json_with_pagination(
    result: &ListResult,
    page: Option<i32>,
    page_size: Option<i32>,
) -> anyhow::Result<String> {
    let items = result
        .items
        .iter()
        .map(|item| JsonFileItem {
            name: item.name.clone(),
            file_type: match item.kind {
                EntryKind::Folder => "folder".to_string(),
                EntryKind::File => "file".to_string(),
            },
            size: item.size,
            modified: item.modified.clone(),
        })
        .collect::<Vec<_>>();

    let output = JsonListOutput {
        path: result.path.clone(),
        page,
        page_size,
        total: result.total,
        items,
    };

    Ok(serde_json::to_string_pretty(&output)?)
}
