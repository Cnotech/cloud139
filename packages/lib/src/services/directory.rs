//! 云端目录解析、遍历和修改操作的前置检查。
use crate::client::api::personal::client::{ListingRetry, PersonalClient};
use crate::client::http::HttpClientWrapper;
use crate::config::{Config, PersonalConfig};
use crate::error::ClientError;
use crate::models::list::parse_personal_time;
use crate::protocol;
use crate::utils::remote_file::is_non_root_file_id;
use crate::utils::remote_path;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct RemoteEntry {
    pub id: String,
    pub folder: bool,
}

pub async fn resolve(config: &Config, path: &str) -> Result<RemoteEntry, ClientError> {
    let config = PersonalConfig::new(config)?;
    let path = remote_path::normalize(path)?;
    if path == "/" {
        return Ok(remote_root().0);
    }
    let client = HttpClientWrapper::new();
    let listing = PersonalClient::connect(config, &client, ListingRetry::None).await?;
    resolve_entry(&listing, &path).await.map(|(entry, _)| entry)
}

fn remote_root() -> (RemoteEntry, Option<i64>) {
    (
        RemoteEntry {
            id: protocol::ROOT_FILE_ID.into(),
            folder: true,
        },
        None,
    )
}

pub(super) async fn resolve_entry(
    listing: &PersonalClient<'_>,
    path: &str,
) -> Result<(RemoteEntry, Option<i64>), ClientError> {
    match listing.resolve_path(path).await? {
        Some(item) => {
            let folder = item.is_folder();
            let id = item.file_id.ok_or(ClientError::InvalidFilePath)?;
            Ok((RemoteEntry { id, folder }, item.size))
        }
        None => Ok(remote_root()),
    }
}

fn require_directory(entry: RemoteEntry) -> Result<String, ClientError> {
    if !entry.folder {
        return Err(crate::error::invalid("目标路径不是目录"));
    }
    Ok(entry.id)
}

pub async fn directory(config: &Config, path: &str) -> Result<String, ClientError> {
    require_directory(resolve(config, path).await?)
}

pub(super) async fn directory_with_listing(
    listing: &PersonalClient<'_>,
    path: &str,
) -> Result<String, ClientError> {
    let (entry, _) = resolve_entry(listing, path).await?;
    require_directory(entry)
}

pub(super) async fn confirm_directory(
    listing: &PersonalClient<'_>,
    path: &str,
    expected_id: &str,
) -> anyhow::Result<()> {
    use anyhow::Context;
    let current = directory_with_listing(listing, path)
        .await
        .with_context(|| format!("无法复核云端目录 {path}"))?;
    anyhow::ensure!(current == expected_id, "云端目录 {path} 的 ID 已变更");
    Ok(())
}

pub(super) async fn ensure_personal_dir(
    listing: &PersonalClient<'_>,
    path: &str,
    parents: bool,
) -> Result<String, ClientError> {
    let mut parent_id = protocol::ROOT_FILE_ID.to_owned();
    let mut parts = path.split('/').filter(|part| !part.is_empty()).peekable();
    let mut seen_ids = HashSet::from([parent_id.clone()]);
    while let Some(name) = parts.next() {
        let last = parts.peek().is_none();
        let id = ensure_personal_child_dir(listing, &parent_id, name, parents, last).await?;
        if !seen_ids.insert(id.clone()) {
            return Err(crate::error::invalid("路径解析返回根目录别名或目录循环"));
        }
        parent_id = id;
    }
    Ok(parent_id)
}

pub(super) async fn ensure_personal_child_dir(
    listing: &PersonalClient<'_>,
    parent_id: &str,
    name: &str,
    parents: bool,
    last: bool,
) -> Result<String, ClientError> {
    let item = if let Some(item) = listing.find_child(parent_id, name).await? {
        if !item.is_folder() {
            return Err(crate::error::invalid(format!("已有同名文件: {name}")));
        }
        if last && !parents {
            return Err(crate::error::invalid(
                "目录已存在；如需已有目录成功，请使用 -p",
            ));
        }
        item
    } else {
        if !parents && !last {
            return Err(ClientError::FileNotFound);
        }
        listing.create_directory(parent_id, name).await?;
        // 重新解析实际目录，防止后续操作误用自动改名后创建的同级目录。
        let item = listing
            .find_child(parent_id, name)
            .await?
            .ok_or_else(|| crate::error::invalid("创建后无法确认目录名称"))?;
        if !item.is_folder() {
            return Err(crate::error::invalid("创建后目录类型冲突"));
        }
        item
    };
    item.file_id
        .filter(|id| is_non_root_file_id(id))
        .ok_or(ClientError::InvalidFilePath)
}

#[derive(Debug)]
pub struct TreeEntry {
    pub path: String,
    pub folder: bool,
    pub size: Option<i64>,
    pub modified: String,
}

pub async fn children(
    config: &Config,
    path: &str,
) -> Result<Vec<(String, RemoteEntry, Option<i64>, String)>, ClientError> {
    let config = PersonalConfig::new(config)?;
    let path = remote_path::normalize(path)?;
    let client = HttpClientWrapper::new();
    let listing = PersonalClient::connect(config, &client, ListingRetry::None).await?;
    let id = directory_with_listing(&listing, &path).await?;
    children_by_id(&listing, &id).await
}

async fn children_by_id(
    listing: &PersonalClient<'_>,
    id: &str,
) -> Result<Vec<(String, RemoteEntry, Option<i64>, String)>, ClientError> {
    listing
        .list_files(id)
        .await?
        .into_iter()
        .map(|item| {
            let folder = item.is_folder();
            let modified = parse_personal_time(item.modified_time().unwrap_or_default());
            Ok((
                item.name.ok_or(ClientError::InvalidFilePath)?,
                RemoteEntry {
                    id: item
                        .file_id
                        .filter(|s| !s.is_empty())
                        .ok_or(ClientError::InvalidFilePath)?,
                    folder,
                },
                item.size,
                modified,
            ))
        })
        .collect()
}

pub async fn tree(config: &Config, root: &str) -> Result<Vec<TreeEntry>, ClientError> {
    let config = PersonalConfig::new(config)?;
    tree_normalized(config, remote_path::normalize(root)?).await
}

pub(super) async fn tree_normalized(
    config: PersonalConfig<&Config>,
    root: String,
) -> Result<Vec<TreeEntry>, ClientError> {
    let client = HttpClientWrapper::new();
    let listing = PersonalClient::connect(config, &client, ListingRetry::None).await?;
    let root_id = directory_with_listing(&listing, &root).await?;
    Ok(tree_with_listing(&listing, root, root_id)
        .await?
        .into_iter()
        .map(|(entry, _)| entry)
        .collect())
}

pub(super) async fn tree_with_listing(
    listing: &PersonalClient<'_>,
    root: String,
    root_id: String,
) -> Result<Vec<(TreeEntry, String)>, ClientError> {
    let mut pending = vec![(root, root_id, 0usize)];
    let mut seen = HashSet::new();
    let mut entries = Vec::new();
    while let Some((path, id, depth)) = pending.pop() {
        if depth > 256 || !seen.insert(id.clone()) {
            return Err(crate::error::invalid("目录循环或层级过深"));
        }
        let mut names = HashSet::new();
        for (name, item, size, modified) in children_by_id(listing, &id).await? {
            remote_path::validate_name(&name)?;
            if !names.insert(name.clone()) {
                return Err(crate::error::invalid("服务端返回重复名称"));
            }
            let child = format!("{}/{name}", path.trim_end_matches('/'));
            if item.folder {
                pending.push((child.clone(), item.id.clone(), depth + 1));
            }
            entries.push((
                TreeEntry {
                    path: child,
                    folder: item.folder,
                    size,
                    modified,
                },
                item.id,
            ));
            if entries.len() > 100_000 {
                return Err(crate::error::invalid("目录条目超过安全上限 100000"));
            }
        }
    }
    entries.sort_by(|(a, _), (b, _)| a.path.cmp(&b.path));
    Ok(entries)
}

pub async fn cp(
    config: &Config,
    sources: &[String],
    target: &str,
    force: bool,
) -> Result<(), ClientError> {
    transfer(config, sources, target, force, false).await
}

pub async fn mv(
    config: &Config,
    sources: &[String],
    target: &str,
    force: bool,
) -> Result<(), ClientError> {
    transfer(config, sources, target, force, true).await
}

pub async fn transfer(
    config: &Config,
    sources: &[String],
    target: &str,
    force: bool,
    moving: bool,
) -> Result<(), ClientError> {
    let config = PersonalConfig::new(config)?;
    if sources.is_empty() {
        return Err(ClientError::NoSourceFiles);
    }
    let target = remote_path::normalize(target)?;
    let sources = sources
        .iter()
        .map(|s| remote_path::non_root(s))
        .collect::<Result<Vec<_>, _>>()?;
    for source in &sources {
        if target == *source || target.starts_with(&format!("{source}/")) {
            return Err(crate::error::invalid("不能复制或移动到自身或子目录"));
        }
    }
    let client = HttpClientWrapper::new();
    let listing = PersonalClient::connect(config, &client, ListingRetry::None).await?;
    let target_id = directory_with_listing(&listing, &target).await?;
    let existing = listing.list_files(&target_id).await?;
    let mut names = HashSet::new();
    let mut ids = Vec::new();
    for source in &sources {
        let (entry, _) = resolve_entry(&listing, source).await?;
        let name = source.rsplit('/').next().unwrap();
        if !names.insert(name.to_owned()) {
            return Err(crate::error::invalid("多个源条目同名"));
        }
        if target_id == entry.id {
            return Err(crate::error::invalid("目标是源目录自身"));
        }
        if moving
            && source
                .rsplit_once('/')
                .map(|(p, _)| if p.is_empty() { "/" } else { p })
                == Some(target.as_str())
        {
            return Err(crate::error::invalid("源目录和目标目录相同"));
        }
        if let Some(conflict) = existing.iter().find(|i| i.name.as_deref() == Some(name))
            && (entry.folder || conflict.is_folder() || !force)
        {
            return Err(ClientError::ForceRequired);
        }
        ids.push(entry.id);
    }
    if moving {
        listing.move_files(&ids, &target_id).await?;
    } else {
        listing.copy_files(&ids, &target_id).await?;
    }
    crate::success!("{}任务已接受", if moving { "移动" } else { "复制" });

    Ok(())
}
