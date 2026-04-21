use crate::cli::commands::list::ListArgs as CliListArgs;
use crate::client::endpoints::{family, group};
use crate::client::{Client, StorageType};
use crate::debug;
use crate::domain::file_item::{EntryKind, FileItem};
use crate::models::PersonalListResp;
use crate::utils::parse_personal_time;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListResult {
    pub path: String,
    pub total: i32,
    pub items: Vec<FileItem>,
}

pub async fn list(
    config: &crate::config::Config,
    args: &CliListArgs,
) -> anyhow::Result<ListResult> {
    let path = if args.path.is_empty() || args.path == "/" {
        "/".to_string()
    } else {
        args.path.trim().to_string()
    };

    let storage_type = config.storage_type();

    match storage_type {
        StorageType::PersonalNew => list_personal(config, &path, args.page, args.page_size).await,
        StorageType::Family => list_family(config, &path, args.page, args.page_size).await,
        StorageType::Group => list_group(config, &path, args.page, args.page_size).await,
    }
}

async fn list_personal(
    config: &crate::config::Config,
    path: &str,
    page: i32,
    page_size: i32,
) -> anyhow::Result<ListResult> {
    let mut config = config.clone();
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
        crate::client::api::get_file_id_by_path(&config, path).await?
    };

    debug!(
        "list_personal: path={}, parent_file_id={}, page_size={}",
        path, parent_file_id, page_size
    );

    let thumbnail_styles = if config.use_large_thumbnail {
        serde_json::json!(["Small", "Large", "Original"])
    } else {
        serde_json::json!(["Small", "Large"])
    };

    let mut next_cursor = String::new();
    let mut all_items = Vec::new();

    loop {
        let body = serde_json::json!({
            "imageThumbnailStyleList": thumbnail_styles,
            "orderBy": "updated_at",
            "orderDirection": "DESC",
            "pageInfo": {
                "pageCursor": next_cursor,
                "pageSize": page_size
            },
            "parentFileId": parent_file_id
        });

        let resp: PersonalListResp =
            crate::client::api::personal_api_request(&config, &url, body, StorageType::PersonalNew)
                .await?;

        if !resp.base.success {
            let msg = resp.base.message.as_deref().unwrap_or("未知错误");
            return Err(crate::client::ClientError::Api(msg.to_string()).into());
        }

        let data = match resp.data {
            Some(d) => d,
            None => {
                return Err(crate::client::ClientError::Api(
                    "获取文件列表失败: 无数据".to_string(),
                )
                .into());
            }
        };

        for item in &data.items {
            let kind = if item.file_type.as_deref() == Some("folder") {
                EntryKind::Folder
            } else {
                EntryKind::File
            };

            let modified = parse_personal_time(
                item.updated_at
                    .as_deref()
                    .or(item.update_date.as_deref())
                    .or(item.last_modified.as_deref())
                    .unwrap_or_default(),
            );

            all_items.push(FileItem {
                name: item.name.clone().unwrap_or_default(),
                kind,
                size: item.size.unwrap_or(0),
                modified,
            });
        }

        next_cursor = data.next_page_cursor.clone().unwrap_or_default();
        if next_cursor.is_empty() {
            break;
        }
    }

    let total = all_items.len() as i32;
    let start = ((page - 1) * page_size) as usize;
    let end = (start + page_size as usize).min(all_items.len());
    let items = if start < all_items.len() {
        all_items[start..end].to_vec()
    } else {
        Vec::new()
    };

    debug!("list_personal: 返回 {} 个条目 (总 {})", items.len(), total);
    Ok(ListResult {
        path: path.to_string(),
        total,
        items,
    })
}

async fn list_family(
    config: &crate::config::Config,
    path: &str,
    page: i32,
    page_size: i32,
) -> anyhow::Result<ListResult> {
    let url = family::orchestration::QUERY_CONTENT_LIST;

    let catalog_id = if path == "/" || path.is_empty() {
        "0".to_string()
    } else {
        path.trim_start_matches('/').to_string()
    };

    let body = crate::models::FamilyListRequest {
        catalog_id: catalog_id.clone(),
        content_sort_type: 0,
        sort_direction: 1,
        page_info: crate::models::PageInfo {
            page_num: page,
            page_size,
        },
    };

    let client = Client::new(config.clone());
    let resp: crate::models::QueryContentListResp = client
        .api_request_post(url, serde_json::to_value(body)?)
        .await?;

    if resp.data.result.result_code != "0" {
        let msg = resp.data.result.result_desc.unwrap_or_default();
        return Err(crate::client::ClientError::Api(msg).into());
    }

    // 家庭云根目录时缓存 path
    let mut config = config.clone();
    if catalog_id == "0" && !resp.data.path.is_empty() {
        config.root_folder_id = Some(resp.data.path.clone());
        let _ = config.save();
    }

    let mut all_items = Vec::new();

    for cat in &resp.data.cloud_catalog_list {
        all_items.push(FileItem {
            name: cat.catalog_name.clone(),
            kind: EntryKind::Folder,
            size: 0,
            modified: cat.last_update_time.clone(),
        });
    }

    for content in &resp.data.cloud_content_list {
        all_items.push(FileItem {
            name: content.content_name.clone(),
            kind: EntryKind::File,
            size: content.content_size,
            modified: content.last_update_time.clone(),
        });
    }

    Ok(ListResult {
        path: path.to_string(),
        total: resp.data.total_count,
        items: all_items,
    })
}

async fn list_group(
    config: &crate::config::Config,
    path: &str,
    page: i32,
    page_size: i32,
) -> anyhow::Result<ListResult> {
    let url = group::orchestration::QUERY_GROUP_CONTENT_LIST;

    let catalog_id = if path == "/" || path.is_empty() {
        "0".to_string()
    } else {
        path.trim_start_matches('/').to_string()
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

    let start_number = (page - 1) * page_size + 1;
    let end_number = page * page_size;

    let body = crate::models::GroupListRequest {
        group_id: config.cloud_id.clone().unwrap_or_default(),
        catalog_id: catalog_id.clone(),
        content_sort_type: 0,
        sort_direction: 1,
        start_number,
        end_number,
        path: group_path.clone(),
    };

    let client = Client::new(config.clone());
    let resp: crate::models::QueryGroupContentListResp = client
        .api_request_post(url, serde_json::to_value(body)?)
        .await?;

    if resp.data.result.result_code != "0" {
        let msg = resp.data.result.result_desc.unwrap_or_default();
        return Err(crate::client::ClientError::Api(msg).into());
    }

    let mut all_items = Vec::new();

    for cat in &resp.data.get_group_content_result.catalog_list {
        all_items.push(FileItem {
            name: cat.catalog_name.clone(),
            kind: EntryKind::Folder,
            size: 0,
            modified: cat.update_time.clone(),
        });
    }

    for content in &resp.data.get_group_content_result.content_list {
        all_items.push(FileItem {
            name: content.content_name.clone(),
            kind: EntryKind::File,
            size: content.content_size,
            modified: content.update_time.clone(),
        });
    }

    Ok(ListResult {
        path: path.to_string(),
        total: resp.data.get_group_content_result.node_count,
        items: all_items,
    })
}
