use crate::client::api::personal::listing::ListPageOptions;
use crate::client::api::personal::{ListingRetry, PersonalClient};
use crate::client::http::HttpClientWrapper;
use crate::debug;
use crate::domain::file_item::{EntryKind, FileItem};
use crate::protocol;
use crate::utils::remote_path;
use crate::utils::time::parse_personal_time;
use futures_util::TryStreamExt;
use std::pin::pin;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListResult {
    pub path: String,
    pub total: i32,
    pub items: Vec<FileItem>,
}

pub struct ListOptions<'a> {
    pub path: &'a str,
    pub page: i32,
    pub page_size: i32,
}

pub async fn list<'a>(
    config: &crate::config::Config,
    options: impl Into<ListOptions<'a>>,
) -> anyhow::Result<ListResult> {
    let args = options.into();
    config.require_personal()?;
    if args.page < 1 || args.page_size < 1 {
        anyhow::bail!("页码和每页数量必须大于 0");
    }
    let path = remote_path::normalize(args.path)?;
    let client = HttpClientWrapper::new();
    let listing = PersonalClient::connect(config, &client, ListingRetry::None).await?;
    let parent = super::directory_service::directory_with_listing(&listing, &path).await?;
    let thumbnails = if config.use_large_thumbnail {
        protocol::THUMBNAILS_WITH_ORIGINAL
    } else {
        protocol::STANDARD_THUMBNAILS
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
    debug!("云端目录列表读取完成 count={} total={}", items.len(), total);
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
    let root = remote_path::normalize(root)?;
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
