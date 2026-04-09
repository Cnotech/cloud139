use crate::client::{Client, ClientError, StorageType};
use crate::models::{
    FamilyListRequest, GroupListRequest, PageInfo, PersonalListResp, QueryContentListResp,
    QueryGroupContentListResp,
};
use crate::utils::pad_with_width;
use crate::{error, success};
use anyhow::Context;
use chrono::NaiveDateTime;
use clap::Parser;
use serde::Serialize;
use std::fs;

#[derive(Parser, Debug)]
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
}

pub async fn execute(args: ListArgs) -> anyhow::Result<()> {
    let path = if args.path.is_empty() || args.path == "/" {
        "/".to_string()
    } else {
        args.path.trim().to_string()
    };
    let mut config = crate::config::Config::load().context("加载配置失败")?;
    let storage_type = config.storage_type();

    match storage_type {
        StorageType::PersonalNew => {
            let host = match &config.personal_cloud_host {
                Some(cached_host) => cached_host.clone(),
                None => {
                    let host = crate::client::api::get_personal_cloud_host(&mut config).await?;
                    config.personal_cloud_host = Some(host.clone());
                    let _ = config.save();
                    host
                }
            };
            let url = format!("{}/file/list", host);

            let parent_file_id = if path == "/" || path.is_empty() {
                "/".to_string()
            } else {
                crate::client::api::get_file_id_by_path(&config, &path).await?
            };

            let mut next_cursor = String::new();
            let mut all_items = Vec::new();

            let thumbnail_styles = if config.use_large_thumbnail {
                serde_json::json!(["Small", "Large", "Original"])
            } else {
                serde_json::json!(["Small", "Large"])
            };

            loop {
                let body = serde_json::json!({
                    "imageThumbnailStyleList": thumbnail_styles,
                    "orderBy": "updated_at",
                    "orderDirection": "DESC",
                    "pageInfo": {
                        "pageCursor": next_cursor,
                        "pageSize": args.page_size
                    },
                    "parentFileId": parent_file_id
                });

                let resp: PersonalListResp =
                    crate::client::api::personal_api_request(&config, &url, body, storage_type)
                        .await?;

                if !resp.base.success {
                    let msg = resp.base.message.as_deref().unwrap_or("未知错误");
                    error!("获取文件列表失败: {}", msg);
                    return Err(ClientError::Api(msg.to_string()).into());
                }

                let data = match resp.data {
                    Some(d) => d,
                    None => {
                        error!("获取文件列表失败: 无数据");
                        return Err(ClientError::Api("获取文件列表失败: 无数据".to_string()).into());
                    }
                };

                if next_cursor.is_empty() {
                    println!("\n文件列表 ({}):", path);
                    println!("{:<40} {:>15} {:<20}", "名称", "大小", "修改时间");
                    println!("{}", "-".repeat(80));
                }

                for item in &data.items {
                    let file_type_str = if item.file_type.as_deref() == Some("folder") {
                        "d"
                    } else {
                        "-"
                    };
                    let size = format_size(item.size.unwrap_or(0));
                    let time = parse_personal_time(
                        item.updated_at
                            .as_deref()
                            .or(item.update_date.as_deref())
                            .or(item.last_modified.as_deref())
                            .unwrap_or_default(),
                    );
                    let name = item.name.as_deref().unwrap_or("");
                    println!(
                        "{} {} {:>15} {:<20}",
                        file_type_str,
                        pad_with_width(name, 38),
                        size,
                        time
                    );

                    all_items.push(JsonFileItem {
                        name: item.name.clone().unwrap_or_default(),
                        file_type: if item.file_type.as_deref() == Some("folder") {
                            "folder".to_string()
                        } else {
                            "file".to_string()
                        },
                        size: item.size.unwrap_or(0),
                        modified: time,
                    });
                }

                next_cursor = data.next_page_cursor.clone().unwrap_or_default();
                if next_cursor.is_empty() {
                    break;
                }
            }

            if let Some(output_path) = &args.output {
                let total = all_items.len() as i32;
                let json_output = JsonListOutput {
                    path: path.clone(),
                    page: Some(args.page),
                    page_size: Some(args.page_size),
                    total,
                    items: all_items,
                };
                let json_str = serde_json::to_string_pretty(&json_output)
                    .map_err(|e| ClientError::Other(e.to_string()))?;
                fs::write(output_path, json_str).map_err(|e| ClientError::Other(e.to_string()))?;
                success!("已输出目录信息到 {}", output_path);
            }
        }
        StorageType::Family => {
            let url = "https://yun.139.com/orchestration/familyCloud-rebuild/content/v1.2/queryContentList";

            let catalog_id = if path == "/" || path.is_empty() {
                "0".to_string()
            } else {
                path.trim_start_matches('/').to_string()
            };

            let body = FamilyListRequest {
                catalog_id: catalog_id.clone(),
                content_sort_type: 0,
                sort_direction: 1,
                page_info: PageInfo {
                    page_num: args.page,
                    page_size: args.page_size,
                },
            };

            let mut config = config.clone();
            let client = Client::new(config.clone());
            let resp: QueryContentListResp = client
                .api_request_post(url, serde_json::to_value(body)?)
                .await?;

            if resp.data.result.result_code != "0" {
                let msg = resp.data.result.result_desc.unwrap_or_default();
                error!("获取文件列表失败: {}", msg);
                return Err(ClientError::Api(msg).into());
            }

            if catalog_id == "0" && !resp.data.path.is_empty() {
                config.root_folder_id = Some(resp.data.path.clone());
                let _ = config.save();
            }

            let mut all_items = Vec::new();

            println!("\n家庭云文件列表 ({}):", args.path);
            println!("{:<40} {:>15} {:<20}", "名称", "大小", "修改时间");
            println!("{}", "-".repeat(80));

            for cat in &resp.data.cloud_catalog_list {
                println!(
                    "d {} {:>15} {:<20}",
                    pad_with_width(&cat.catalog_name, 38),
                    "-",
                    cat.last_update_time
                );
                all_items.push(JsonFileItem {
                    name: cat.catalog_name.clone(),
                    file_type: "folder".to_string(),
                    size: 0,
                    modified: cat.last_update_time.clone(),
                });
            }

            for content in &resp.data.cloud_content_list {
                let size = format_size(content.content_size);
                println!(
                    "- {} {:>15} {:<20}",
                    pad_with_width(&content.content_name, 38),
                    size,
                    content.last_update_time
                );
                all_items.push(JsonFileItem {
                    name: content.content_name.clone(),
                    file_type: "file".to_string(),
                    size: content.content_size,
                    modified: content.last_update_time.clone(),
                });
            }

            println!("\n总计: {} 项", resp.data.total_count);

            if let Some(output_path) = &args.output {
                let total = all_items.len() as i32;
                let json_output = JsonListOutput {
                    path: path.clone(),
                    page: None,
                    page_size: None,
                    total,
                    items: all_items,
                };
                let json_str = serde_json::to_string_pretty(&json_output)
                    .map_err(|e| ClientError::Other(e.to_string()))?;
                fs::write(output_path, json_str).map_err(|e| ClientError::Other(e.to_string()))?;
                success!("已输出目录信息到 {}", output_path);
            }
        }
        StorageType::Group => {
            let url = "https://yun.139.com/orchestration/group-rebuild/content/v1.0/queryGroupContentList";

            let catalog_id = if args.path == "/" || args.path.is_empty() {
                "0".to_string()
            } else {
                args.path.trim_start_matches('/').to_string()
            };

            let root_folder_id = config
                .root_folder_id
                .clone()
                .unwrap_or_else(|| "root:".to_string());
            let group_path = if catalog_id == "0" || catalog_id.is_empty() {
                root_folder_id.clone()
            } else {
                format!("{}/{}", root_folder_id.trim_end_matches(':'), catalog_id)
            };

            let start_number = (args.page - 1) * args.page_size + 1;
            let end_number = args.page * args.page_size;

            let body = GroupListRequest {
                group_id: config.cloud_id.clone().unwrap_or_default(),
                catalog_id: catalog_id.clone(),
                content_sort_type: 0,
                sort_direction: 1,
                start_number,
                end_number,
                path: group_path.clone(),
            };

            let client = Client::new(config);
            let resp: QueryGroupContentListResp = client
                .api_request_post(url, serde_json::to_value(body)?)
                .await?;

            if resp.data.result.result_code != "0" {
                let msg = resp.data.result.result_desc.unwrap_or_default();
                error!("获取文件列表失败: {}", msg);
                return Err(ClientError::Api(msg).into());
            }

            let mut all_items = Vec::new();

            println!("\n群组云文件列表 ({}):", args.path);
            println!("{:<40} {:>15} {:<20}", "名称", "大小", "修改时间");
            println!("{}", "-".repeat(80));

            for cat in &resp.data.get_group_content_result.catalog_list {
                println!(
                    "d {} {:>15} {:<20}",
                    pad_with_width(&cat.catalog_name, 38),
                    "-",
                    cat.update_time
                );
                all_items.push(JsonFileItem {
                    name: cat.catalog_name.clone(),
                    file_type: "folder".to_string(),
                    size: 0,
                    modified: cat.update_time.clone(),
                });
            }

            for content in &resp.data.get_group_content_result.content_list {
                let size = format_size(content.content_size);
                println!(
                    "- {} {:>15} {:<20}",
                    pad_with_width(&content.content_name, 38),
                    size,
                    content.update_time
                );
                all_items.push(JsonFileItem {
                    name: content.content_name.clone(),
                    file_type: "file".to_string(),
                    size: content.content_size,
                    modified: content.update_time.clone(),
                });
            }

            println!(
                "\n总计: {} 项",
                resp.data.get_group_content_result.node_count
            );

            if let Some(output_path) = &args.output {
                let total = all_items.len() as i32;
                let json_output = JsonListOutput {
                    path: args.path.clone(),
                    page: None,
                    page_size: None,
                    total,
                    items: all_items,
                };
                let json_str = serde_json::to_string_pretty(&json_output)
                    .map_err(|e| ClientError::Other(e.to_string()))?;
                fs::write(output_path, json_str).map_err(|e| ClientError::Other(e.to_string()))?;
                success!("已输出目录信息到 {}", output_path);
            }
        }
    }

    Ok(())
}

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

pub fn parse_personal_time(time_str: &str) -> String {
    if time_str.is_empty() {
        return String::new();
    }

    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(time_str) {
        return dt.format("%Y-%m-%d %H:%M:%S").to_string();
    }

    if let Ok(dt) = NaiveDateTime::parse_from_str(time_str, "%Y-%m-%dT%H:%M:%S%.f") {
        return dt.format("%Y-%m-%d %H:%M:%S").to_string();
    }

    time_str.to_string()
}
