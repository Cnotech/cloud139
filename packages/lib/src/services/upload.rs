use crate::client::api::personal::transfer::put_upload_part;
use crate::client::api::personal::{ListingRetry, PersonalClient};
use crate::client::api::upload::UploadFile;
use crate::client::http::HttpClientWrapper;
use crate::error::{ClientError, invalid};
use crate::models::list::PersonalFileItem;
use crate::models::sync_item::{FileEntry, SyncEntryKind};
use crate::progress::Progress;
use crate::protocol;
use crate::services::sync::remote::remote_sha256;
use crate::services::sync::{
    CloudSyncFile, CloudSyncScan, SYNC_LIST_RETRY, SYNC_UPLOAD_PREFIX, UPLOAD_BACKUP_PREFIX,
};
use crate::utils::path::safe_local;
use crate::utils::remote_path;
use crate::utils::{
    crypto::is_sha256_hex,
    remote_file::is_non_root_file_id,
    upload_parts::{checked_part_count, part_offset_and_size},
};
use anyhow::Context;
use std::collections::HashMap;
use std::path::Path;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncSeekExt, SeekFrom};

use super::directory as dirs;

pub async fn upload_path(
    config: &crate::config::Config,
    local: &str,
    remote: &str,
    force: bool,
    make_progress: impl FnOnce(&str, u64) -> Option<Progress>,
) -> anyhow::Result<()> {
    let local_path = Path::new(local);
    if !local_path.exists() {
        crate::error!("文件不存在: {}", local);
        return Err(ClientError::FileNotFound.into());
    }

    safe_local(local_path)?;
    if local_path.is_dir() {
        return upload_directory(
            config,
            local_path,
            remote,
            local.ends_with('/') || local.ends_with('\\'),
            force,
        )
        .await;
    }
    if !local_path.is_file() {
        anyhow::bail!("只支持普通文件或目录");
    }
    let file_name = local_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    let file_size = checked_upload_size(local_path)
        .map_err(|error| anyhow::anyhow!("无法上传 {}: {}", local, error))?;

    let remote_dir = if remote == "/" {
        "".to_string()
    } else {
        remote.trim_end_matches('/').to_string()
    };

    crate::debug!(
        "准备上传文件 local={} remote={}/{}",
        local,
        remote_dir,
        file_name
    );
    crate::debug!("已读取本地文件大小 size_bytes={}", file_size);

    let pb = make_progress(file_name, file_size as u64);

    upload(
        config,
        local_path,
        &remote_dir,
        file_name,
        file_size,
        force,
        pb,
    )
    .await?;

    Ok(())
}

/// Re-read metadata before hashing or any destructive sync operation.
pub fn checked_upload_size(path: &Path) -> Result<i64, ClientError> {
    safe_local(path)?;
    let metadata = std::fs::metadata(path)?;
    if !metadata.is_file() {
        return Err(invalid("只支持普通文件；目录请使用递归上传"));
    }
    i64::try_from(metadata.len()).map_err(|_| invalid("文件过大"))
}

pub async fn upload(
    config: &crate::config::Config,
    local_path: &Path,
    remote_path: &str,
    file_name: &str,
    file_size: i64,
    force: bool,
    pb: Option<Progress>,
) -> Result<(), ClientError> {
    config.require_personal()?;
    remote_path::validate_name(file_name)?;
    if file_size < 0 {
        return Err(invalid("无效文件大小"));
    }
    checked_upload_size(local_path)?;
    let remote_path = remote_path::normalize(remote_path)?;
    let http_client = HttpClientWrapper::new();
    let listing = PersonalClient::connect(config, &http_client, ListingRetry::DIRECTORY).await?;
    let parent = dirs::directory_with_listing(&listing, &remote_path).await?;
    upload_file(&listing, local_path, &parent, file_name, force, pb).await
}

/// Upload `dir` itself, or its contents when the local argument has a trailing separator.
pub async fn upload_directory(
    config: &crate::config::Config,
    local: &Path,
    remote: &str,
    contents: bool,
    force: bool,
) -> anyhow::Result<()> {
    config.require_personal()?;
    safe_local(local)?;
    if !local.is_dir() {
        return Err(invalid("递归上传源必须是目录").into());
    }
    let entries = super::local_scan::scan_items(local, true, &[])?;
    let remote = remote_path::normalize(remote)?;
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
    // Validate the complete local plan before any cloud mutation.
    for entry in &entries {
        for name in entry.rel_path.split('/') {
            remote_path::validate_name(name)?;
        }
        safe_local(&local.join(&entry.rel_path))?;
    }
    upload_personal_directory(config, local, &root, entries, force).await
}

async fn upload_personal_directory(
    config: &crate::config::Config,
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

async fn upload_file(
    listing: &PersonalClient<'_>,
    path: &Path,
    parent_id: &str,
    name: &str,
    force: bool,
    pb: Option<Progress>,
) -> Result<(), ClientError> {
    let size = checked_upload_size(path)?;
    let items = listing.list_files(parent_id).await?;
    for item in items
        .iter()
        .filter(|item| item.name.as_deref() == Some(name))
    {
        if item.is_folder() {
            return Err(invalid("云端同名目录阻止上传文件"));
        }
        if !force {
            return Err(ClientError::ForceRequired);
        }
    }
    let saved_name = upload_to_parent(listing, path, parent_id, name, size, pb.clone()).await?;
    crate::success!("上传完成: {saved_name}");
    if let Some(pb) = pb {
        pb.finish();
    }
    Ok(())
}

pub(super) struct SyncUploadPlan<'a> {
    pub(super) host: &'a str,
    pub(super) scan: Option<&'a CloudSyncScan>,
    pub(super) rel_path: &'a str,
    pub(super) parent_id: &'a str,
    pub(super) remote_parent: &'a str,
    pub(super) file_name: &'a str,
    pub(super) replace_directory_id: Option<&'a str>,
    pub(super) hash_missing: Option<&'a mut bool>,
    pub(super) file_changed: &'a mut bool,
}

fn finish_upload(name: &str, pb: &Option<Progress>) {
    crate::success!("上传完成: {name}");
    if let Some(bar) = pb {
        bar.finish();
    }
}

/// Upload a unique sibling before replacing the destination.
pub(super) async fn upload_sync_to_parent(
    config: &crate::config::Config,
    local_path: &std::path::Path,
    plan: SyncUploadPlan<'_>,
    pb: Option<Progress>,
) -> anyhow::Result<String> {
    let SyncUploadPlan {
        host,
        parent_id,
        remote_parent,
        file_name,
        replace_directory_id,
        hash_missing,
        scan,
        rel_path,
        file_changed,
    } = plan;
    let file_size = checked_upload_size(local_path)?;
    remote_path::validate_name(file_name)?;
    // --verify 发布内容寻址对象（64-hex 文件名）前，先确认本地内容哈希与文件名一致；
    // 不一致说明本地内容已损坏，直接失败并保留远端旧副本（此时还没创建任何远端对象）。
    let verify_content_addressed = hash_missing.is_some() && is_sha256_hex(file_name);
    if verify_content_addressed {
        let path = local_path.to_path_buf();
        let local_hash =
            tokio::task::spawn_blocking(move || crate::utils::crypto::calc_file_sha256(&path))
                .await??;
        anyhow::ensure!(
            local_hash.eq_ignore_ascii_case(file_name),
            "内容寻址对象 {file_name} 的实际内容哈希 {local_hash} 与文件名不一致，已保留远端旧副本"
        );
    }
    let temporary_name = format!(
        "{SYNC_UPLOAD_PREFIX}{}",
        crate::utils::rand::generate_rand_str(24)
    );
    let client = HttpClientWrapper::new();
    let personal = PersonalClient::with_host(config, host, &client, ListingRetry::None);
    let before = personal.list_files(parent_id).await?;
    if before
        .iter()
        .any(|item| item.name.as_deref() == Some(&temporary_name))
    {
        anyhow::bail!("临时上传名称冲突，旧条目未替换");
    }
    let previous = before
        .iter()
        .find(|item| item.name.as_deref() == Some(file_name));
    let expected_id = if let Some(scan) = scan {
        scan.expected_id(rel_path)?
    } else {
        previous
            .map(|item| {
                item.file_id
                    .as_deref()
                    .filter(|id| is_non_root_file_id(id))
                    .context("旧条目缺少文件 ID")
            })
            .transpose()?
    };
    let expected_file = scan.and_then(|scan| scan.files.get(rel_path));
    check_destination(&before, file_name, expected_id, expected_file)?;
    anyhow::ensure!(
        replace_directory_id.is_none() || scan.is_some(),
        "替换目录需要扫描记录"
    );
    let uploaded = transfer_to_parent(
        &personal,
        local_path,
        parent_id,
        &temporary_name,
        file_size,
        pb.clone(),
    )
    .await
    .with_context(|| {
        format!("同步上传 {file_name} 失败，旧条目未替换；临时名称 {temporary_name}")
    })?;
    let saved_name = uploaded.name.as_deref().unwrap_or(&temporary_name);
    // The source may change after the preflight read. This is the digest used by
    // upload initialization and remote verification; bind publication to it too.
    if verify_content_addressed {
        anyhow::ensure!(
            uploaded.sha256.eq_ignore_ascii_case(file_name),
            "内容寻址对象 {file_name} 的上传内容哈希 {} 与文件名不一致；临时名称 {saved_name}，旧条目未替换",
            uploaded.sha256
        );
    }
    let promote = async {
        let id = uploaded.id.as_str();
        if before
            .iter()
            .any(|item| item.file_id.as_deref() == Some(id))
        {
            anyhow::bail!("临时文件 ID 已存在，旧条目未替换");
        }
        let mut files = confirm_uploaded_name(&personal, parent_id, id, saved_name).await?;
        check_destination(&files, file_name, expected_id, expected_file)?;
        if let Some(hash_missing) = hash_missing {
            let item = files
                .iter()
                .find(|item| {
                    item.file_id.as_deref() == Some(id) && item.name.as_deref() == Some(saved_name)
                })
                .context("找不到已确认的临时文件")?;
            let remote = CloudSyncFile::from(item);
            if remote.size.is_some_and(|size| size != uploaded.size) {
                anyhow::bail!("上传后校验失败：临时文件大小不一致");
            }
            let digest =
                remote_sha256(config, host, &remote, Some(uploaded.size), hash_missing).await?;
            if digest != uploaded.sha256 {
                anyhow::bail!("上传后校验失败：临时文件 SHA-256 不一致");
            }
            if remote.sha256.is_none() {
                files = confirm_uploaded_name(&personal, parent_id, id, saved_name).await?;
                check_destination(&files, file_name, expected_id, expected_file)?;
            }
        }
        let listing = personal.with_listing_retry(SYNC_LIST_RETRY);
        if let Some(dir_id) = replace_directory_id {
            scan.context("替换目录需要扫描记录")?
                .confirm_subtree(&listing, rel_path, dir_id)
                .await?;
            files = confirm_uploaded_name(&personal, parent_id, id, saved_name).await?;
            check_destination(&files, file_name, expected_id, expected_file)?;
        }
        dirs::confirm_directory(&listing, remote_parent, parent_id).await?;
        let installed = install_uploaded_file(
            &personal,
            parent_id,
            id,
            file_name,
            &files,
            replace_directory_id,
            file_changed,
        )
        .await?;
        finish_upload(&installed, &pb);
        Ok(id.to_owned())
    }
    .await
    .with_context(|| format!("临时名称 {saved_name}，替换 {file_name} 未完成"))?;
    Ok(promote)
}

fn check_destination(
    files: &[PersonalFileItem],
    name: &str,
    expected_id: Option<&str>,
    expected_file: Option<&CloudSyncFile>,
) -> anyhow::Result<()> {
    let mut matches = files
        .iter()
        .filter(|item| item.name.as_deref() == Some(name));
    let unchanged = match (matches.next(), expected_id) {
        (None, None) => true,
        (Some(item), Some(id)) => {
            item.file_id.as_deref() == Some(id)
                && matches.next().is_none()
                && expected_file
                    .is_none_or(|file| !item.is_folder() && CloudSyncFile::from(item) == *file)
        }
        _ => false,
    };
    anyhow::ensure!(unchanged, "目标 {name} 在扫描后发生变化");
    Ok(())
}

async fn upload_to_parent(
    personal: &PersonalClient<'_>,
    local_path: &std::path::Path,
    parent_file_id: &str,
    file_name: &str,
    file_size: i64,
    pb: Option<Progress>,
) -> Result<String, ClientError> {
    let uploaded = transfer_to_parent(
        personal,
        local_path,
        parent_file_id,
        file_name,
        file_size,
        pb,
    )
    .await?;
    if let Some(name) = uploaded.name {
        return Ok(name);
    }
    let files = personal
        .with_listing_retry(ListingRetry::None)
        .list_files(parent_file_id)
        .await?;
    files
        .into_iter()
        .find(|item| item.file_id.as_deref() == Some(&uploaded.id) && !item.is_folder())
        .and_then(|item| item.name)
        .ok_or_else(|| ClientError::Api("上传已提交，但无法确认云端文件名称".into()))
}

struct UploadReceipt {
    name: Option<String>,
    id: String,
    size: u64,
    sha256: String,
}

async fn transfer_to_parent(
    personal: &PersonalClient<'_>,
    local_path: &std::path::Path,
    parent_file_id: &str,
    file_name: &str,
    file_size: i64,
    pb: Option<Progress>,
) -> Result<UploadReceipt, ClientError> {
    if file_size < 0 {
        return Err(ClientError::InvalidFilePath);
    }
    let file_size = checked_upload_size(local_path)?;
    let part_size = personal.upload_part_size(file_size);
    checked_part_count(file_size, part_size)?;
    crate::debug!("开始计算文件 SHA-256");
    let path = local_path.to_owned();
    let content_hash =
        tokio::task::spawn_blocking(move || crate::utils::crypto::calc_file_sha256(path))
            .await
            .map_err(|error| ClientError::Other(format!("计算文件哈希失败: {error}")))??;
    let response = personal
        .create_upload(UploadFile {
            local_path,
            file_name,
            file_size,
            parent_file_id,
            content_hash: &content_hash,
            part_size,
        })
        .await?;
    let receipt = UploadReceipt {
        name: response.name,
        id: response.file_id,
        size: file_size as u64,
        sha256: content_hash,
    };
    if let Some(pending) = response.pending {
        crate::debug!("开始上传分片");
        upload_parts(UploadPartsParams {
            personal,
            local_path,
            upload_id: &pending.upload_id,
            file_id: &receipt.id,
            file_size,
            content_hash: &receipt.sha256,
            part_size,
            initial_parts: &pending.parts,
            pb,
        })
        .await?;
    } else {
        crate::debug!("服务端确认秒传或创建空文件");
    }
    Ok(receipt)
}

struct UploadPartsParams<'a> {
    personal: &'a PersonalClient<'a>,
    local_path: &'a std::path::Path,
    upload_id: &'a str,
    file_id: &'a str,
    file_size: i64,
    content_hash: &'a str,
    part_size: i64,
    initial_parts: &'a [crate::models::upload::PersonalPartInfo],
    pb: Option<Progress>,
}

async fn upload_parts(params: UploadPartsParams<'_>) -> Result<(), ClientError> {
    let personal = params.personal;
    let local_path = params.local_path;
    let upload_id = params.upload_id;
    let file_id = params.file_id;
    let file_size = params.file_size;
    let content_hash = params.content_hash;
    let part_size = params.part_size;

    let file = tokio::fs::File::open(local_path).await?;
    let data_client = HttpClientWrapper::new();
    if file.metadata().await?.len() < file_size as u64 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "本地文件在上传前被截断",
        )
        .into());
    }
    let part_count = checked_part_count(file_size, part_size)?;

    for batch_start in (0..part_count).step_by(protocol::UPLOAD_PARTS_PER_REQUEST as usize) {
        let batch_end = (batch_start + protocol::UPLOAD_PARTS_PER_REQUEST).min(part_count);
        let upload_urls = if batch_start == 0 {
            params
                .initial_parts
                .iter()
                .map(|part| (part.part_number, part.upload_url.clone()))
                .collect()
        } else {
            personal
                .upload_urls(
                    file_id,
                    upload_id,
                    batch_start..batch_end,
                    part_size,
                    file_size,
                )
                .await?
        };
        for i in batch_start..batch_end {
            let (offset, read_size) = part_offset_and_size(i, part_size, file_size);
            let mut part = file.try_clone().await?;
            part.seek(SeekFrom::Start(offset as u64)).await?;
            let part_number = (i + 1) as i32;
            crate::debug!("正在上传分片 part={}/{}", part_number, part_count);
            upload_part(
                &data_client,
                &upload_urls,
                part_number,
                part,
                read_size as u64,
                params.pb.clone(),
            )
            .await?;
        }
    }

    personal
        .complete_upload(file_id, upload_id, content_hash)
        .await
}

async fn upload_part(
    client: &HttpClientWrapper,
    upload_urls: &HashMap<i32, String>,
    part_number: i32,
    reader: impl AsyncRead + Send + Unpin + 'static,
    size: u64,
    pb: Option<Progress>,
) -> Result<(), ClientError> {
    let upload_url = upload_urls
        .get(&part_number)
        .ok_or_else(|| ClientError::Api(format!("找不到分片 {part_number} 的上传 URL")))?;
    let stream = futures_util::stream::try_unfold(
        (reader, size, pb),
        |(mut reader, remaining, pb)| async move {
            if remaining == 0 {
                return Ok::<_, std::io::Error>(None);
            }
            let mut chunk = vec![0; remaining.min(64 * 1024) as usize];
            reader.read_exact(&mut chunk).await?;
            let read = chunk.len() as u64;
            if let Some(pb) = &pb {
                pb.inc(read);
            }
            Ok(Some((chunk, (reader, remaining - read, pb))))
        },
    );
    put_upload_part(client, upload_url, part_number, size, stream).await
}

async fn install_uploaded_file(
    personal: &PersonalClient<'_>,
    parent_file_id: &str,
    file_id: &str,
    file_name: &str,
    files: &[PersonalFileItem],
    replace_directory_id: Option<&str>,
    file_changed: &mut bool,
) -> Result<String, ClientError> {
    let saved_name = files
        .iter()
        .find(|item| item.file_id.as_deref() == Some(file_id) && !item.is_folder())
        .and_then(|item| item.name.as_deref())
        .filter(|name| !name.is_empty())
        .ok_or_else(|| ClientError::Api("分片上传已提交，但无法确认新文件名称".into()))?;
    if saved_name == file_name {
        *file_changed = true;
        return Ok(saved_name.to_owned());
    }

    let mut conflicts = files
        .iter()
        .filter(|item| item.name.as_deref() == Some(file_name));
    let old_file = conflicts.next();
    if let Some(item) = old_file {
        let old_id = item
            .file_id
            .as_deref()
            .filter(|id| is_non_root_file_id(id) && *id != file_id)
            .ok_or(ClientError::InvalidFilePath)?;
        if conflicts.next().is_some() || item.is_folder().then_some(old_id) != replace_directory_id
        {
            return Err(ClientError::Api(format!(
                "同名条目无法安全替换，新文件保存在 {saved_name}"
            )));
        }
        let backup_name = format!(
            "{UPLOAD_BACKUP_PREFIX}{}",
            crate::utils::rand::generate_rand_str(20)
        );
        personal
            .rename(old_id, &backup_name)
            .await
            .map_err(|error| {
                ClientError::Api(format!(
                    "无法为旧文件保留备份，新文件保存在 {saved_name}: {error}"
                ))
            })?;
        personal.rename(file_id, file_name).await.map_err(|error| {
            ClientError::Api(format!(
                "无法重命名新文件；旧文件保留为 {backup_name}，请检查新文件 {saved_name}: {error}"
            ))
        })?;
        let published = confirm_uploaded_name(personal, parent_file_id, file_id, file_name)
            .await
            .map_err(|error| {
                ClientError::Api(format!(
                    "无法确认新文件名称；旧文件备份 {backup_name} 未删除: {error}"
                ))
            })?;
        *file_changed = true;
        if !published.iter().any(|entry| {
            entry.name.as_deref() == Some(&backup_name) && entry.file_id.as_deref() == Some(old_id)
        }) {
            return Err(ClientError::Api(format!(
                "旧文件备份 {backup_name} 已变更，停止回收"
            )));
        }
        personal.trash(old_id).await.map_err(|error| {
            ClientError::Api(format!(
                "新文件已重命名为 {file_name}，但旧文件备份 {backup_name} 未确认移到回收站: {error}"
            ))
        })?;
    } else {
        personal.rename(file_id, file_name).await?;
        confirm_uploaded_name(personal, parent_file_id, file_id, file_name).await?;
        *file_changed = true;
    }
    Ok(file_name.to_owned())
}

async fn confirm_uploaded_name(
    personal: &PersonalClient<'_>,
    parent_id: &str,
    file_id: &str,
    name: &str,
) -> Result<Vec<PersonalFileItem>, ClientError> {
    for attempt in 0..5 {
        let files = personal.list_files(parent_id).await?;
        let mut matches = files
            .iter()
            .filter(|item| item.name.as_deref() == Some(name));
        if matches
            .next()
            .is_some_and(|item| item.file_id.as_deref() == Some(file_id) && !item.is_folder())
            && matches.next().is_none()
            && files
                .iter()
                .filter(|item| item.file_id.as_deref() == Some(file_id))
                .count()
                == 1
        {
            return Ok(files);
        }
        if attempt < 4 {
            tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;
        }
    }
    Err(ClientError::Api(format!(
        "无法在云端列表确认新文件 {name} 的身份"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::api::headers::PC_CLIENT_INFO;
    use httpmock::prelude::*;
    use serde_json::json;

    #[test]
    fn part_count_handles_large_sizes_and_rejects_invalid_layouts() {
        assert_eq!(checked_part_count(i64::MAX, i64::MAX).unwrap(), 1);
        assert_eq!(checked_part_count(0, 100).unwrap(), 0);
        assert_eq!(checked_part_count(101, 100).unwrap(), 2);
        for (size, part) in [(1, 0), (1, -1), (-1, 100), (i64::MAX, 1)] {
            assert!(checked_part_count(size, part).is_err());
        }
    }

    #[tokio::test]
    async fn large_upload_initialization_uses_pc_identity_and_preserves_entitlement_errors() {
        let server = MockServer::start();
        let config = crate::config::Config {
            personal_cloud_host: Some(server.base_url()),
            ..Default::default()
        };
        let client = HttpClientWrapper::new();
        let personal =
            PersonalClient::with_host(&config, server.base_url(), &client, ListingRetry::None);
        for gib in [5_i64, 8, 50, 500] {
            let size = gib * 1024 * 1024 * 1024 + 1;
            let request = server.mock(|when, then| {
                when.method(POST).path("/file/create")
                    .header("x-yun-client-info", PC_CLIENT_INFO)
                    .json_body_includes(format!("{{\"size\":{size}}}"));
                then.json_body(json!({"success":true,"data":{"fileId":"new","uploadId":"upload","partInfos":[{"partNumber":1,"uploadUrl":"https://example.invalid/part"}]}}));
            });
            let response = personal
                .create_upload(UploadFile {
                    local_path: std::path::Path::new("large.bin"),
                    file_name: "large.bin",
                    file_size: size,
                    parent_file_id: "/",
                    content_hash: "fictional-hash",
                    part_size: personal.upload_part_size(size),
                })
                .await
                .unwrap();
            assert_eq!(response.file_id, "new");
            request.assert_calls(1);
        }
        let rejected = server.mock(|when, then| {
            when.method(POST)
                .path("/file/create")
                .json_body_includes(r#"{"size":1}"#)
                .header("x-yun-client-info", PC_CLIENT_INFO);
            then.json_body(json!({"success":false,"code":"04010319","message":"权益不足"}));
        });
        let error = personal
            .create_upload(UploadFile {
                local_path: std::path::Path::new("small.bin"),
                file_name: "small.bin",
                file_size: 1,
                parent_file_id: "/",
                content_hash: "fictional-hash",
                part_size: personal.upload_part_size(1),
            })
            .await
            .err()
            .expect("server should reject the upload");
        assert_eq!(
            error.to_string(),
            "API 错误: 创建上传任务失败: 权益不足（错误码: 04010319）；文件大小 1 字节，请核对账号当前的单文件上传额度"
        );
        rejected.assert_calls(1);
    }

    #[tokio::test]
    async fn ordinary_and_silver_limits_are_enforced_by_server_not_local_policy() {
        let server = MockServer::start();
        let gib = 1024_i64 * 1024 * 1024;
        let client = HttpClientWrapper::new();
        for (plan, limit) in [("ordinary", 5 * gib), ("silver", 8 * gib)] {
            let config = crate::config::Config {
                authorization: format!("fictional-{plan}-token"),
                ..Default::default()
            };
            let personal =
                PersonalClient::with_host(&config, server.base_url(), &client, ListingRetry::None);
            for size in [5 * gib, 5 * gib + 1, 8 * gib, 8 * gib + 1] {
                let accepted = size <= limit;
                let part_size = 100 * 1024 * 1024;
                let count = (size + part_size - 1) / part_size;
                let parts: Vec<_> = (0..count)
                    .map(|i| {
                        json!({
                            "partNumber": i + 1,
                            "partSize": part_size.min(size - i * part_size),
                            "parallelHashCtx": {"partOffset": i * part_size}
                        })
                    })
                    .collect();
                let mut request = server.mock(|when, then| {
                    when.method(POST).path("/file/create")
                        .header("Authorization", format!("Basic fictional-{plan}-token"))
                        .header("x-yun-client-info", PC_CLIENT_INFO)
                        .json_body(json!({
                            "contentHash": "fictional-hash", "contentHashAlgorithm": "SHA256",
                            "contentType": "application/octet-stream", "parallelUpload": false,
                            "partInfos": parts, "size": size, "parentFileId": "/",
                            "name": "boundary.bin", "type": "file", "fileRenameMode": "auto_rename"
                        }));
                    then.json_body(if accepted {
                        json!({"success":true,"data":{"fileId":"test-file","uploadId":"test-upload",
                            "partInfos":[{"partNumber":1,"uploadUrl":"https://example.invalid/part"}]}})
                    } else {
                        json!({"success":false,"code":"04010319","message":"权益不足"})
                    });
                });
                let result = personal
                    .create_upload(UploadFile {
                        local_path: std::path::Path::new("boundary.bin"),
                        file_name: "boundary.bin",
                        file_size: size,
                        parent_file_id: "/",
                        content_hash: "fictional-hash",
                        part_size: personal.upload_part_size(size),
                    })
                    .await;
                assert_eq!(result.is_ok(), accepted, "{plan}: {size}");
                if let Err(error) = result {
                    assert!(error.to_string().contains("04010319"));
                }
                request.assert_calls(1);
                request.delete();
            }
        }
    }

    #[tokio::test]
    async fn upload_reuses_initial_urls_and_fetches_only_the_remaining_batch() {
        let server = MockServer::start();
        let config = crate::config::Config::default();
        let source = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(source.path(), vec![b'x'; 101]).unwrap();
        let part = server.mock(|when, then| {
            when.method(PUT)
                .path("/initial")
                .body("x")
                .header("Content-Type", "application/octet-stream")
                .header("Content-Length", "1")
                .header("Origin", "https://yun.139.com")
                .header("Referer", "https://yun.139.com/");
            then.status(200);
        });
        let tail = server.mock(|when, then| {
            when.method(PUT).path("/tail").body("x");
            then.status(200);
        });
        let first_reissue = server.mock(|when, then| {
            when.method(POST)
                .path("/file/getUploadUrl")
                .json_body_includes(r#"{"partInfos":[{"partNumber":1}]}"#);
            then.status(500);
        });
        let urls = server.mock(|when, then| {
            when.method(POST).path("/file/getUploadUrl")
                .json_body(json!({"fileId":"new","uploadId":"upload",
                    "partInfos":[{"partNumber":101,"partSize":1,"parallelHashCtx":{"partOffset":100}}],
                    "commonAccountInfo":{"account":config.account,"accountType":1}}));
            then.json_body(json!({"success":true,"data":{"partInfos":[{"partNumber":101,"uploadUrl":server.url("/tail")}]}}));
        });
        let complete = server.mock(|when, then| {
            when.path("/file/complete");
            then.json_body(json!({"success":true}));
        });
        let initial: Vec<_> = (1..=100)
            .map(|part_number| crate::models::upload::PersonalPartInfo {
                part_number,
                upload_url: server.url("/initial"),
            })
            .collect();
        let client = HttpClientWrapper::new();
        let personal =
            PersonalClient::with_host(&config, server.base_url(), &client, ListingRetry::None);
        upload_parts(UploadPartsParams {
            personal: &personal,
            local_path: source.path(),
            upload_id: "upload",
            file_id: "new",
            file_size: 101,
            content_hash: "fictional-hash",
            part_size: 1,
            initial_parts: &initial,
            pb: None,
        })
        .await
        .unwrap();
        part.assert_calls(100);
        tail.assert_calls(1);
        first_reissue.assert_calls(0);
        urls.assert_calls(1);
        complete.assert_calls(1);
    }

    #[tokio::test]
    async fn truncated_local_file_never_uploads_a_partial_part_or_completes() {
        let server = MockServer::start();
        let config = crate::config::Config::default();
        let source = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(source.path(), b"x").unwrap();
        let urls = server.mock(|when, then| {
            when.method(POST).path("/file/getUploadUrl");
            then.json_body(json!({"success":true,"data":{"partInfos":[{"partNumber":1,"uploadUrl":server.url("/part")}]}}));
        });
        let part = server.mock(|when, then| {
            when.method(PUT).path("/part");
            then.status(200);
        });
        let complete = server.mock(|when, then| {
            when.path("/file/complete");
            then.json_body(json!({"success":true}));
        });
        let client = HttpClientWrapper::new();
        let personal =
            PersonalClient::with_host(&config, server.base_url(), &client, ListingRetry::None);
        let error = upload_parts(UploadPartsParams {
            personal: &personal,
            local_path: source.path(),
            upload_id: "upload",
            file_id: "new",
            file_size: 2,
            content_hash: "fictional-hash",
            part_size: 2,
            initial_parts: &[crate::models::upload::PersonalPartInfo {
                part_number: 1,
                upload_url: server.url("/part"),
            }],
            pb: None,
        })
        .await
        .unwrap_err();
        assert!(
            matches!(error, ClientError::Io(e) if e.kind() == std::io::ErrorKind::UnexpectedEof)
        );
        urls.assert_calls(0);
        part.assert_calls(0);
        complete.assert_calls(0);
    }

    #[tokio::test]
    async fn acknowledged_but_unconfirmed_rename_never_trashes_backup() {
        let server = MockServer::start();
        let config = crate::config::Config {
            personal_cloud_host: Some(server.base_url()),
            ..Default::default()
        };
        let list = server.mock(|when, then| {
            when.method(POST).path("/file/list");
            then.json_body(json!({"success":true,"data":{"items":[
                {"fileId":"old","name":"report.txt","type":"file"},
                {"fileId":"new","name":"temporary.txt","type":"file"}
            ]}}));
        });
        let rename = server.mock(|when, then| {
            when.path("/file/update");
            then.json_body(json!({"success":true}));
        });
        let trash = server.mock(|when, then| {
            when.path("/recyclebin/batchTrash");
            then.json_body(json!({"success":true}));
        });
        let client = HttpClientWrapper::new();
        let personal =
            PersonalClient::with_host(&config, server.base_url(), &client, ListingRetry::None);
        let files = personal.list_files("/").await.unwrap();
        let mut file_changed = false;
        let error = install_uploaded_file(
            &personal,
            "/",
            "new",
            "report.txt",
            &files,
            None,
            &mut file_changed,
        )
        .await
        .unwrap_err();
        assert!(!file_changed);
        assert!(error.to_string().contains("未删除"));
        assert!(error.to_string().contains(".cloud139-backup-"));
        list.assert_calls(6);
        rename.assert_calls(2);
        trash.assert_calls(0);
    }

    #[tokio::test]
    async fn failed_upload_conflict_renames_do_not_trash_existing_file() {
        for fail_backup in [true, false] {
            let server = MockServer::start();
            let config = crate::config::Config {
                authorization: "fictional".into(),
                account: "13800138000".into(),
                personal_cloud_host: Some(server.base_url()),
                ..Default::default()
            };
            let listing = server.mock(|when, then| {
                when.method(POST).path("/file/list");
                then.json_body(json!({"success":true,"data":{"items":[
                    {"name":"report.txt","fileId":"old","type":"file"},
                    {"name":"report-copy.txt","fileId":"new","type":"file"}
                ]}}));
            });
            let backup = server.mock(|when, then| {
                when.method(POST)
                    .path("/file/update")
                    .json_body_includes(r#"{"fileId":"old"}"#);
                then.json_body(json!({"success":!fail_backup}));
            });
            let rename = server.mock(|when, then| {
                when.method(POST)
                    .path("/file/update")
                    .json_body_includes(r#"{"fileId":"new","name":"report.txt"}"#);
                then.json_body(json!({"success":false,"message":"rename rejected"}));
            });
            let trash = server.mock(|when, then| {
                when.method(POST).path("/recyclebin/batchTrash");
                then.json_body(json!({"success":true}));
            });
            let client = HttpClientWrapper::new();
            let personal =
                PersonalClient::with_host(&config, server.base_url(), &client, ListingRetry::None);
            let files = personal.list_files("/").await.unwrap();
            let mut file_changed = false;
            let result = install_uploaded_file(
                &personal,
                "/",
                "new",
                "report.txt",
                &files,
                None,
                &mut file_changed,
            )
            .await;
            assert!(!file_changed);
            let message = result.unwrap_err().to_string();
            assert!(message.contains("report-copy.txt"));
            if !fail_backup {
                assert!(message.contains(".cloud139-backup-"));
            }
            listing.assert_calls(1);
            backup.assert_calls(1);
            rename.assert_calls(usize::from(!fail_backup));
            trash.assert_calls(0);
        }
    }
}
