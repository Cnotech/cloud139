use crate::commands::list::ListArgs as CliListArgs;
use crate::debug;
use crate::domain::StorageType;
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
    if args.page < 1 || args.page_size < 1 {
        anyhow::bail!("页码和每页数量必须大于 0");
    }
    let normalized = super::directory_service::normalize(&args.path)?;
    let path = if normalized.is_empty() || normalized == "/" {
        "/".to_string()
    } else {
        normalized
    };

    let storage_type = config.storage_type();

    match storage_type {
        StorageType::PersonalNew => list_personal(config, &path, args.page, args.page_size).await,
        StorageType::Family => list_legacy(config, &path, args.page, args.page_size).await,
        StorageType::Group => list_legacy(config, &path, args.page, args.page_size).await,
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
            host
        }
    };
    let url = format!("{}/file/list", host);

    let parent_file_id = super::directory_service::directory(&config, path).await?;

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

    let mut seen_cursors = std::collections::HashSet::new();
    loop {
        if !seen_cursors.insert(next_cursor.clone()) {
            anyhow::bail!("分页游标重复，已停止列目录");
        }
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
            let kind = if super::sync_service::should_treat_personal_item_as_folder(item) {
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
                content_hash: item.content_hash.clone(),
                content_hash_algorithm: item.content_hash_algorithm.clone(),
            });
        }

        next_cursor = data.next_page_cursor.clone().unwrap_or_default();
        if next_cursor.is_empty() {
            break;
        }
    }

    let total = all_items.len() as i32;
    let start = ((page as usize) - 1).saturating_mul(page_size as usize);
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

async fn list_legacy(
    config: &crate::config::Config,
    path: &str,
    page: i32,
    page_size: i32,
) -> anyhow::Result<ListResult> {
    let dir = super::legacy_directory::resolve(config, path).await?;
    let all = super::legacy_directory::list_all(config, &dir).await?;
    let total = all.len() as i32;
    let start = (page as usize - 1).saturating_mul(page_size as usize);
    let items = all
        .into_iter()
        .skip(start)
        .take(page_size as usize)
        .map(|i| FileItem {
            name: i.name,
            kind: if i.folder {
                EntryKind::Folder
            } else {
                EntryKind::File
            },
            size: i.size.unwrap_or(0),
            modified: i.modified,
            content_hash: None,
            content_hash_algorithm: None,
        })
        .collect();
    Ok(ListResult {
        path: path.into(),
        total,
        items,
    })
}

/// Full listing grouped by directory, including empty directories.
pub async fn list_recursive(
    config: &crate::config::Config,
    root: &str,
) -> anyhow::Result<Vec<ListResult>> {
    use super::directory_service as dirs;
    use crate::domain::file_item::{EntryKind, FileItem};
    let root = dirs::normalize(root)?;
    let entries = dirs::tree(config, &root).await?;
    let mut grouped = std::collections::BTreeMap::<String, Vec<FileItem>>::new();
    grouped.insert(root, vec![]);
    for entry in entries {
        if entry.folder {
            grouped.entry(entry.path.clone()).or_default();
        }
        let (parent, name) = entry.path.rsplit_once('/').unwrap();
        grouped
            .entry(if parent.is_empty() {
                "/".into()
            } else {
                parent.into()
            })
            .or_default()
            .push(FileItem {
                name: name.into(),
                kind: if entry.folder {
                    EntryKind::Folder
                } else {
                    EntryKind::File
                },
                size: entry.size.unwrap_or(0),
                modified: entry.modified,
                content_hash: None,
                content_hash_algorithm: None,
            });
    }
    Ok(grouped
        .into_iter()
        .map(|(path, items)| ListResult {
            path,
            total: items.len() as i32,
            items,
        })
        .collect())
}
