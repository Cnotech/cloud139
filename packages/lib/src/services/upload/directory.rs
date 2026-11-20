use super::{check_upload_conflicts, checked_upload_size, finish_upload, upload_to_parent};
use crate::client::api::personal::client::{ListingRetry, PersonalClient};
use crate::client::http::HttpClientWrapper;
use crate::config::{Config, personal::PersonalConfig};
use crate::error::{ClientError, invalid};
use crate::models::sync_item::{FileEntry, SyncEntryKind};
use crate::services::directory as dirs;
use crate::services::local_scan::scan_items;
use crate::utils::{path::safe_local, remote_path};
use std::collections::{HashMap, hash_map::Entry};
use std::path::Path;

pub(super) async fn upload_personal_tree(
    config: PersonalConfig<&Config>,
    local: &Path,
    remote: &str,
    contents: bool,
    force: bool,
) -> anyhow::Result<()> {
    let remote = remote_path::normalize(remote)?;
    safe_local(local)?;
    if !local.is_dir() {
        return Err(invalid("递归上传源必须是目录").into());
    }
    let entries = scan_items(local, true, &[])?;
    let root = if contents {
        remote
    } else {
        let name = local
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or(ClientError::InvalidFilePath)?;
        remote_path::validate_name(name)?;
        format!("{}/{}", remote.trim_end_matches('/'), name)
    };
    // 修改云端前，先校验完整的本地上传计划。
    for entry in &entries {
        for name in entry.rel_path.split('/') {
            remote_path::validate_name(name)?;
        }
        safe_local(&local.join(&entry.rel_path))?;
    }
    upload_directory_entries(config, local, &root, entries, force).await
}

async fn upload_directory_entries(
    config: PersonalConfig<&Config>,
    local: &Path,
    root: &str,
    entries: Vec<FileEntry>,
    force: bool,
) -> anyhow::Result<()> {
    if root == "/" && entries.is_empty() {
        return Ok(());
    }
    let http_client = HttpClientWrapper::new();
    let listing = PersonalClient::connect(config, &http_client, ListingRetry::DIRECTORY).await?;
    let root_id = dirs::ensure_personal_dir(&listing, root, true).await?;
    let mut directories = HashMap::from([(root.to_owned(), root_id)]);
    let mut names_by_parent = HashMap::<String, HashMap<String, bool>>::new();
    let mut uploaded = 0;
    let mut failures = Vec::new();
    for entry in entries {
        let destination = format!("{}/{}", root.trim_end_matches('/'), entry.rel_path);
        let (parent, name) = destination.rsplit_once('/').unwrap();
        let parent = if parent.is_empty() { "/" } else { parent };
        if entry.kind == SyncEntryKind::Directory {
            let id = dirs::ensure_personal_dir(&listing, &destination, true)
                .await
                .map_err(|error| {
                    anyhow::anyhow!(
                        "创建目录 {} 失败；此前条目可能已完成: {:#}",
                        entry.rel_path,
                        anyhow::Error::new(error)
                    )
                })?;
            if let Some(names) = directories
                .get(parent)
                .and_then(|id| names_by_parent.get_mut(id))
            {
                names.insert(name.to_owned(), true);
            }
            directories.insert(destination, id);
            continue;
        }
        let parent_id = directories
            .get(parent)
            .ok_or_else(|| invalid("上传计划缺少父目录"))?;
        let path = local.join(&entry.rel_path);
        let result = async {
            let size = checked_upload_size(&path)?;
            let names = match names_by_parent.entry(parent_id.clone()) {
                Entry::Occupied(entry) => entry.into_mut(),
                Entry::Vacant(entry) => {
                    let mut names = HashMap::<String, bool>::new();
                    for item in listing.list_files(parent_id).await? {
                        let folder = item.is_folder();
                        if let Some(name) = item.name {
                            names
                                .entry(name)
                                .and_modify(|kind| *kind |= folder)
                                .or_insert(folder);
                        }
                    }
                    entry.insert(names)
                }
            };
            check_upload_conflicts(names.get(name).copied(), force)?;
            match upload_to_parent(&listing, &path, parent_id, name, size, None).await {
                Ok(saved_name) => {
                    finish_upload(&saved_name, &None);
                    names.entry(saved_name).or_insert(false);
                    Ok(())
                }
                Err(error) => {
                    // 云端可能已接受写入，下一个文件须重新列举父目录。
                    names_by_parent.remove(parent_id);
                    Err(error)
                }
            }
        }
        .await;
        match result {
            Ok(()) => uploaded += 1,
            Err(error) => {
                let message = format!(
                    "上传 {} 失败；此前条目可能已完成: {}",
                    entry.rel_path, error
                );
                crate::error!("{}", message);
                failures.push(message);
            }
        }
    }
    if !failures.is_empty() {
        anyhow::bail!(
            "目录上传未完成: 成功 {} 个，失败 {} 个；{}",
            uploaded,
            failures.len(),
            failures.join("；")
        );
    }
    crate::success!("目录上传完成: 成功 {} 个文件", uploaded);
    Ok(())
}
