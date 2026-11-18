use crate::client::api::{HttpClientWrapper, ListPageOptions, ListingRetry, PersonalClient};
use crate::commands::list::ListArgs as CliListArgs;
use crate::debug;
use crate::domain::file_item::{EntryKind, FileItem};
use crate::utils::parse_personal_time;
use futures_util::TryStreamExt;
use std::pin::pin;

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
    config.require_personal()?;
    if args.page < 1 || args.page_size < 1 {
        anyhow::bail!("页码和每页数量必须大于 0");
    }
    let path = super::directory_service::normalize(&args.path)?;
    let client = HttpClientWrapper::new();
    let listing = PersonalClient::connect(config, &client, ListingRetry::None).await?;
    let parent = super::directory_service::directory_with_listing(&listing, &path).await?;
    let thumbnails: &[&str] = if config.use_large_thumbnail {
        &["Small", "Large", "Original"]
    } else {
        &["Small", "Large"]
    };
    let mut pages = pin!(listing.pages(
        &parent,
        ListPageOptions {
            size: args.page_size,
            thumbnails: Some(thumbnails),
        }
    ));
    let page_size = args.page_size as usize;
    let start = (args.page as usize - 1).saturating_mul(page_size);
    let mut total: usize = 0;
    let mut items = Vec::new();
    while let Some(page) = pages.try_next().await? {
        let offset = start.saturating_sub(total);
        total += page.len();
        items.extend(
            page.into_iter()
                .skip(offset)
                .take(page_size - items.len())
                .map(|item| {
                    let kind = if item.is_folder() {
                        EntryKind::Folder
                    } else {
                        EntryKind::File
                    };
                    let modified = parse_personal_time(item.modified_time().unwrap_or_default());
                    FileItem {
                        name: item.name.unwrap_or_default(),
                        kind,
                        size: item.size.unwrap_or(0),
                        modified,
                        content_hash: item.content_hash,
                        content_hash_algorithm: item.content_hash_algorithm,
                    }
                }),
        );
    }
    debug!("list_personal: 返回 {} 个条目 (总 {})", items.len(), total);
    Ok(ListResult {
        path,
        total: i32::try_from(total)?,
        items,
    })
}

/// Full listing grouped by directory, including empty directories.
pub async fn list_recursive(
    config: &crate::config::Config,
    root: &str,
) -> anyhow::Result<Vec<ListResult>> {
    use super::directory_service as dirs;
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
