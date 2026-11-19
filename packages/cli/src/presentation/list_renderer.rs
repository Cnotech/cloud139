use crate::print;
use cloud139::domain::file_item::EntryKind;
use cloud139::services::list_service::ListResult;
use serde::Serialize;

pub(crate) fn format_size(size: i64) -> String {
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

pub(crate) fn render_terminal(result: &ListResult) {
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
struct JsonListOutput {
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    page: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    page_size: Option<i32>,
    total: i32,
    items: Vec<JsonFileItem>,
}

#[derive(Serialize)]
struct JsonFileItem {
    name: String,
    #[serde(rename = "type")]
    file_type: String,
    size: i64,
    modified: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    content_hash_algorithm: Option<String>,
}

pub(crate) fn json_output(result: &ListResult, pagination: Option<(i32, i32)>) -> impl Serialize {
    let (page, page_size) = match pagination {
        Some((page, page_size)) => (Some(page), Some(page_size)),
        None => (None, None),
    };
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
            content_hash: item.content_hash.clone(),
            content_hash_algorithm: item.content_hash_algorithm.clone(),
        })
        .collect::<Vec<_>>();

    JsonListOutput {
        path: result.path.clone(),
        page,
        page_size,
        total: result.total,
        items,
    }
}
