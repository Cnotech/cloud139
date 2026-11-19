use super::upload_file;
use crate::client::api::personal::client::{ListingRetry, PersonalClient};
use crate::client::http::HttpClientWrapper;
use crate::config::{Config, personal::PersonalConfig};
use crate::error::{ClientError, invalid};
use crate::models::sync_item::{FileEntry, SyncEntryKind};
use crate::services::directory as dirs;
use crate::services::local_scan::scan_items;
use crate::utils::{path::safe_local, remote_path};
use std::collections::HashMap;
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
    let mut uploaded = 0;
    let mut failures = Vec::new();
    for entry in entries {
        let destination = format!("{}/{}", root.trim_end_matches('/'), entry.rel_path);
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
            directories.insert(destination, id);
            continue;
        }
        let (parent, name) = destination.rsplit_once('/').unwrap();
        let parent = if parent.is_empty() { "/" } else { parent };
        let parent_id = directories
            .get(parent)
            .ok_or_else(|| invalid("上传计划缺少父目录"))?;
        let path = local.join(&entry.rel_path);
        let result = upload_file(&listing, &path, parent_id, name, force, None).await;
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
