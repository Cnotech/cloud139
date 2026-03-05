use clap::Parser;
use crate::client::{Client, ClientError, StorageType};
use crate::models::{PersonalListResp, FamilyListRequest, QueryContentListResp, GroupListRequest, QueryGroupContentListResp};

#[derive(Parser, Debug)]
pub struct ListArgs {
    #[arg(default_value = "/", help = "远程目录路径")]
    pub path: String,
    
    #[arg(short, long, default_value = "1", help = "页码")]
    pub page: i32,
    
    #[arg(short, long, default_value = "100", help = "每页数量")]
    pub page_size: i32,
}

pub async fn execute(args: ListArgs) -> Result<(), ClientError> {
    let mut config = crate::config::Config::load().map_err(|e| ClientError::Config(e))?;
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
            
            let parent_file_id = if args.path == "/" || args.path.is_empty() {
                "".to_string()
            } else {
                crate::client::api::get_file_id_by_path(&config, &args.path).await?
            };

            let mut next_cursor = String::new();
            
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

                let resp: PersonalListResp = crate::client::api::personal_api_request(&config, &url, body, storage_type).await?;

                if !resp.base.success {
                    println!("获取文件列表失败: {}", resp.base.message);
                    return Ok(());
                }

                if next_cursor.is_empty() {
                    println!("\n文件列表 ({}):", args.path);
                    println!("{:<40} {:>15} {:<20}", "名称", "大小", "修改时间");
                    println!("{}", "-".repeat(80));
                }

                for item in &resp.data.items {
                    let file_type = if item.file_type == "folder" { "d" } else { "-" };
                    let size = format_size(item.size);
                    let time = item.update_date.clone().or(item.last_modified.clone()).unwrap_or_default();
                    println!("{:<1} {:<38} {:>15} {:<20}", file_type, item.name, size, time);
                }

                next_cursor = resp.data.next_page_cursor.clone();
                if next_cursor.is_empty() {
                    break;
                }
            }
        }
        StorageType::Family => {
            let url = "https://yun.139.com/orchestration/familyCloud-rebuild/content/v1.2/queryContentList";
            
            let catalog_id = if args.path == "/" || args.path.is_empty() {
                "0".to_string()
            } else {
                args.path.clone()
            };

            let body = FamilyListRequest {
                catalog_id,
                sort_type: 1,
                page_number: args.page,
                page_size: args.page_size,
            };

            let client = Client::new(config);
            let resp: QueryContentListResp = client.api_request_post(url, serde_json::to_value(body)?).await?;

            if resp.data.result.result_code != "0" {
                println!("获取文件列表失败: {}", resp.data.result.result_desc.unwrap_or_default());
                return Ok(());
            }

            println!("\n家庭云文件列表 ({}):", args.path);
            println!("{:<40} {:>15} {:<20}", "名称", "大小", "修改时间");
            println!("{}", "-".repeat(80));

            for cat in &resp.data.cloud_catalog_list {
                println!("{:<1} {:<38} {:>15} {:<20}", "d", cat.catalog_name, "-", cat.last_update_time);
            }

            for content in &resp.data.cloud_content_list {
                let size = format_size(content.content_size);
                println!("{:<1} {:<38} {:>15} {:<20}", "-", content.content_name, size, content.last_update_time);
            }

            println!("\n总计: {} 项", resp.data.total_count);
        }
        StorageType::Group => {
            let url = "https://yun.139.com/orchestration/group-rebuild/content/v1.0/queryGroupContentList";
            
            let catalog_id = if args.path == "/" || args.path.is_empty() {
                "0".to_string()
            } else {
                args.path.clone()
            };

            let body = GroupListRequest {
                catalog_id,
                sort_type: 1,
                page_number: args.page,
                page_size: args.page_size,
            };

            let client = Client::new(config);
            let resp: QueryGroupContentListResp = client.api_request_post(url, serde_json::to_value(body)?).await?;

            if resp.data.result.result_code != "0" {
                println!("获取文件列表失败: {}", resp.data.result.result_desc.unwrap_or_default());
                return Ok(());
            }

            println!("\n群组云文件列表 ({}):", args.path);
            println!("{:<40} {:>15} {:<20}", "名称", "大小", "修改时间");
            println!("{}", "-".repeat(80));

            for cat in &resp.data.get_group_content_result.catalog_list {
                println!("{:<1} {:<38} {:>15} {:<20}", "d", cat.catalog_name, "-", cat.update_time);
            }

            for content in &resp.data.get_group_content_result.content_list {
                let size = format_size(content.content_size);
                println!("{:<1} {:<38} {:>15} {:<20}", "-", content.content_name, size, content.update_time);
            }

            println!("\n总计: {} 项", resp.data.get_group_content_result.node_count);
        }
    }

    Ok(())
}

fn format_size(size: i64) -> String {
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
