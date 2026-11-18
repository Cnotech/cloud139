//! Shared path validation, directory traversal, and mutation preflight.
use crate::client::{ClientError, api};
use crate::config::Config;
use crate::domain::StorageType;
use crate::services::sync_service::should_treat_personal_item_as_folder as is_folder;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub use crate::utils::remote_path::{invalid, non_root, normalize, validate_name};

#[derive(Debug, Clone)]
pub struct RemoteEntry {
    pub id: String,
    pub folder: bool,
}

pub async fn resolve(config: &Config, path: &str) -> Result<RemoteEntry, ClientError> {
    let path = normalize(path)?;
    if config.storage_type() != StorageType::PersonalNew {
        let item = super::legacy_directory::resolve(config, &path).await?;
        return Ok(RemoteEntry {
            id: item.id,
            folder: item.folder,
        });
    }
    let mut current = RemoteEntry {
        id: "/".into(),
        folder: true,
    };
    if path == "/" {
        return Ok(current);
    }
    if path.split('/').count() > 258 {
        return Err(invalid("目录层级过深"));
    }
    let mut cached = config.clone();
    let host = api::get_personal_cloud_host(&mut cached).await?;
    let mut seen_ids = HashSet::from(["/".to_owned()]);
    for part in path.split('/').filter(|p| !p.is_empty()) {
        if !current.folder {
            return Err(invalid("父路径不是目录"));
        }
        let item = api::find_personal_child(
            &cached,
            &host,
            &current.id,
            part,
            &api::HttpClientWrapper::new(),
        )
        .await?
        .ok_or(ClientError::FileNotFound)?;
        let folder = is_folder(&item);
        let id = item
            .file_id
            .filter(|s| !s.is_empty())
            .ok_or(ClientError::InvalidFilePath)?;
        if !seen_ids.insert(id.clone()) {
            return Err(invalid("路径解析返回根目录别名或目录循环"));
        }
        current = RemoteEntry { id, folder };
    }
    Ok(current)
}

pub async fn directory(config: &Config, path: &str) -> Result<String, ClientError> {
    let entry = resolve(config, path).await?;
    if !entry.folder {
        return Err(invalid("目标路径不是目录"));
    }
    Ok(entry.id)
}

pub async fn ensure_dir(config: &Config, path: &str, parents: bool) -> Result<(), ClientError> {
    let path = normalize(path)?;
    if path == "/" {
        return if parents {
            Ok(())
        } else {
            Err(ClientError::CannotOperateOnRoot)
        };
    }
    if config.storage_type() != StorageType::PersonalNew {
        return super::legacy_directory::ensure_dir(config, &path, parents).await;
    }
    let mut cached = config.clone();
    let host = api::get_personal_cloud_host(&mut cached).await?;
    let http_client = api::HttpClientWrapper::new();
    let listing = api::PersonalListing {
        config: &cached,
        host: &host,
        http_client: &http_client,
        retry_transient: false,
    };
    ensure_personal_dir(&listing, &path, parents).await?;
    Ok(())
}

pub(super) async fn ensure_personal_dir(
    listing: &api::PersonalListing<'_>,
    path: &str,
    parents: bool,
) -> Result<String, ClientError> {
    let mut parent_id = "/".to_owned();
    let mut parts = path.split('/').filter(|part| !part.is_empty()).peekable();
    let mut seen_ids = HashSet::from([parent_id.clone()]);
    while let Some(name) = parts.next() {
        let last = parts.peek().is_none();
        let id = ensure_personal_child_dir(listing, &parent_id, name, parents, last).await?;
        if !seen_ids.insert(id.clone()) {
            return Err(invalid("路径解析返回根目录别名或目录循环"));
        }
        parent_id = id;
    }
    Ok(parent_id)
}

pub(super) async fn ensure_personal_child_dir(
    listing: &api::PersonalListing<'_>,
    parent_id: &str,
    name: &str,
    parents: bool,
    last: bool,
) -> Result<String, ClientError> {
    let item = if let Some(item) = listing.find_child(parent_id, name).await? {
        if !is_folder(&item) {
            return Err(invalid(format!("已有同名文件: {name}")));
        }
        if last && !parents {
            return Err(invalid("目录已存在；如需已有目录成功，请使用 -p"));
        }
        item
    } else {
        if !parents && !last {
            return Err(ClientError::FileNotFound);
        }
        let response: crate::models::PersonalUploadResp = api::personal_api_request(
            listing.config,
            &format!("{}/file/create", listing.host),
            serde_json::json!({
                "parentFileId": parent_id, "name": name, "description": "", "type": "folder"
            }),
            StorageType::PersonalNew,
        )
        .await?;
        if !response.base.success {
            return Err(ClientError::Api(
                response
                    .base
                    .message
                    .unwrap_or_else(|| "创建目录失败".into()),
            ));
        }
        // Resolve the actual directory: never proceed using an automatically renamed sibling.
        let item = listing
            .find_child(parent_id, name)
            .await?
            .ok_or_else(|| invalid("创建后无法确认目录名称"))?;
        if !is_folder(&item) {
            return Err(invalid("创建后目录类型冲突"));
        }
        item
    };
    item.file_id
        .filter(|id| !id.is_empty() && id != "/")
        .ok_or(ClientError::InvalidFilePath)
}

/// Reject symlinks at every existing component, including user-supplied parents.
pub fn safe_local(path: &Path) -> Result<(), ClientError> {
    let absolute = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()?.join(path)
    };
    #[cfg(target_os = "macos")]
    let absolute = {
        let mut resolved = absolute.clone();
        for alias in ["/var", "/tmp", "/etc"] {
            if let Ok(rest) = absolute.strip_prefix(alias) {
                resolved = std::fs::canonicalize(alias)?.join(rest);
                break;
            }
        }
        resolved
    };
    let mut current = PathBuf::new();
    for component in absolute.components() {
        if component == std::path::Component::ParentDir {
            return Err(invalid("本地路径不允许 .."));
        }
        current.push(component);
        match std::fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(invalid(format!("拒绝符号链接: {}", current.display())));
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
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
    if config.storage_type() != StorageType::PersonalNew {
        let dir = super::legacy_directory::resolve(config, path).await?;
        return Ok(super::legacy_directory::list_all(config, &dir)
            .await?
            .into_iter()
            .map(|i| {
                (
                    i.name,
                    RemoteEntry {
                        id: i.id,
                        folder: i.folder,
                    },
                    i.size,
                    i.modified,
                )
            })
            .collect());
    }
    let id = directory(config, path).await?;
    api::list_personal_files(config, &id)
        .await?
        .into_iter()
        .map(|item| {
            let folder = is_folder(&item);
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
                crate::utils::parse_personal_time(item.updated_at.as_deref().unwrap_or_default()),
            ))
        })
        .collect()
}

pub async fn tree(config: &Config, root: &str) -> Result<Vec<TreeEntry>, ClientError> {
    let root = normalize(root)?;
    let root_id = directory(config, &root).await?;
    let mut pending = vec![(root, root_id, 0usize)];
    let mut seen = HashSet::new();
    let mut entries = Vec::new();
    while let Some((path, id, depth)) = pending.pop() {
        if depth > 256 || !seen.insert(id) {
            return Err(invalid("目录循环或层级过深"));
        }
        let mut names = HashSet::new();
        for (name, item, size, modified) in children(config, &path).await? {
            validate_name(&name)?;
            if !names.insert(name.clone()) {
                return Err(invalid("服务端返回重复名称"));
            }
            let child = format!("{}/{name}", path.trim_end_matches('/'));
            if item.folder {
                pending.push((child.clone(), item.id, depth + 1));
            }
            entries.push(TreeEntry {
                path: child,
                folder: item.folder,
                size,
                modified,
            });
            if entries.len() > 100_000 {
                return Err(invalid("目录条目超过安全上限 100000"));
            }
        }
    }
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(entries)
}

pub async fn transfer(
    config: &Config,
    sources: &[String],
    target: &str,
    force: bool,
    moving: bool,
) -> Result<(), ClientError> {
    if sources.is_empty() {
        return Err(ClientError::NoSourceFiles);
    }
    let target = normalize(target)?;
    let sources = sources
        .iter()
        .map(|s| non_root(s))
        .collect::<Result<Vec<_>, _>>()?;
    for source in &sources {
        if target == *source || target.starts_with(&format!("{source}/")) {
            return Err(invalid("不能复制或移动到自身或子目录"));
        }
    }
    if config.storage_type() != StorageType::PersonalNew {
        if moving {
            return super::legacy_directory::move_group(config, &sources, &target).await;
        }
        return Err(invalid("家庭/群组复制协议未可靠核实，已拒绝操作"));
    }
    let target_id = directory(config, &target).await?;
    let existing = api::list_personal_files(config, &target_id).await?;
    let mut names = HashSet::new();
    let mut ids = Vec::new();
    for source in &sources {
        let entry = resolve(config, source).await?;
        let name = source.rsplit('/').next().unwrap();
        if !names.insert(name.to_owned()) {
            return Err(invalid("多个源条目同名"));
        }
        if target_id == entry.id {
            return Err(invalid("目标是源目录自身"));
        }
        if moving
            && source
                .rsplit_once('/')
                .map(|(p, _)| if p.is_empty() { "/" } else { p })
                == Some(target.as_str())
        {
            return Err(invalid("源目录和目标目录相同"));
        }
        if let Some(conflict) = existing.iter().find(|i| i.name.as_deref() == Some(name))
            && (entry.folder || is_folder(conflict) || !force)
        {
            return Err(ClientError::ForceRequired);
        }
        ids.push(entry.id);
    }
    let mut cached = config.clone();
    let host = api::get_personal_cloud_host(&mut cached).await?;
    let endpoint = if moving { "batchMove" } else { "batchCopy" };
    let resp: serde_json::Value = api::personal_api_request(
        &cached,
        &format!("{host}/file/{endpoint}"),
        serde_json::json!({ "fileIds": ids, "toParentFileId": target_id }),
        StorageType::PersonalNew,
    )
    .await?;
    if !mutation_succeeded(&resp) {
        return Err(ClientError::Api(
            "批量操作失败（可能部分完成），请检查云端状态".into(),
        ));
    }
    crate::success!("{}任务已接受", if moving { "移动" } else { "复制" });

    Ok(())
}

/// Some batch responses carry per-item success flags under data.
pub fn mutation_succeeded(response: &serde_json::Value) -> bool {
    fn no_failure(value: &serde_json::Value) -> bool {
        match value {
            serde_json::Value::Object(map) => {
                map.get("success").and_then(serde_json::Value::as_bool) != Some(false)
                    && map.values().all(no_failure)
            }
            serde_json::Value::Array(items) => items.iter().all(no_failure),
            _ => true,
        }
    }
    response.get("success").and_then(serde_json::Value::as_bool) == Some(true)
        && no_failure(response)
}
