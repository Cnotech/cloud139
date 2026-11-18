use crate::client::api::upload::{
    PART_BATCH_SIZE, UploadFile, checked_part_count, part_offset_and_size,
};
use crate::client::{ClientError, api};
use crate::models::PersonalFileItem;

use crate::services::sync_service::{CloudSyncFile, SYNC_UPLOAD_PREFIX, UPLOAD_BACKUP_PREFIX};
use crate::utils::logger::{pb_debug, pb_success};
use anyhow::Context;
use indicatif::ProgressBar;
use tokio::io::{AsyncSeekExt, SeekFrom};

fn finish_upload(name: &str, pb: &Option<ProgressBar>) {
    pb_success(&format!("上传完成: {name}"), pb);
    if let Some(bar) = pb {
        bar.finish_and_clear();
    }
}

pub(in crate::services) struct SyncUploadPlan<'a> {
    pub host: &'a str,
    pub scan: Option<&'a super::super::sync_service::CloudSyncScan>,
    pub rel_path: &'a str,
    pub parent_id: &'a str,
    pub remote_parent: &'a str,
    pub file_name: &'a str,
    pub replace_directory_id: Option<&'a str>,
    pub hash_missing: Option<&'a mut bool>,
    pub file_changed: &'a mut bool,
}

/// Upload a unique sibling before replacing the destination.
pub(in crate::services) async fn upload_sync_to_parent(
    config: &crate::config::Config,
    local_path: &std::path::Path,
    plan: SyncUploadPlan<'_>,
    pb: Option<ProgressBar>,
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
    let file_size = super::super::upload_service::checked_upload_size(local_path)?;
    super::super::directory_service::validate_name(file_name)?;
    // --verify 发布内容寻址对象（64-hex 文件名）前，先确认本地内容哈希与文件名一致；
    // 不一致说明本地内容已损坏，直接失败并保留远端旧副本（此时还没创建任何远端对象）。
    let verify_content_addressed = hash_missing.is_some() && is_content_addressed(file_name);
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
    let client = api::HttpClientWrapper::new();
    let personal = api::PersonalClient::with_host(config, host, &client, api::ListingRetry::None);
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
                    .filter(|id| !id.is_empty() && *id != "/")
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
            let remote = super::super::sync_service::CloudSyncFile::from(item);
            if remote.size.is_some_and(|size| size != uploaded.size) {
                anyhow::bail!("上传后校验失败：临时文件大小不一致");
            }
            let digest = super::super::sync_verify::remote_sha256(
                config,
                host,
                &remote,
                Some(uploaded.size),
                hash_missing,
            )
            .await?;
            if digest != uploaded.sha256 {
                anyhow::bail!("上传后校验失败：临时文件 SHA-256 不一致");
            }
            if remote.sha256.is_none() {
                files = confirm_uploaded_name(&personal, parent_id, id, saved_name).await?;
                check_destination(&files, file_name, expected_id, expected_file)?;
            }
        }
        let listing = personal.with_listing_retry(super::super::sync_service::SYNC_LIST_RETRY);
        if let Some(dir_id) = replace_directory_id {
            scan.context("替换目录需要扫描记录")?
                .confirm_subtree(&listing, rel_path, dir_id)
                .await?;
            files = confirm_uploaded_name(&personal, parent_id, id, saved_name).await?;
            check_destination(&files, file_name, expected_id, expected_file)?;
        }
        super::super::directory_service::confirm_directory(&listing, remote_parent, parent_id)
            .await?;
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

/// 内容寻址对象：文件名即内容的 SHA-256（64 个十六进制字符）。
fn is_content_addressed(name: &str) -> bool {
    name.len() == 64 && name.bytes().all(|byte| byte.is_ascii_hexdigit())
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

pub(in crate::services) async fn upload_to_parent(
    personal: &api::PersonalClient<'_>,
    local_path: &std::path::Path,
    parent_file_id: &str,
    file_name: &str,
    file_size: i64,
    pb: Option<ProgressBar>,
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
        .with_listing_retry(api::ListingRetry::None)
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
    personal: &api::PersonalClient<'_>,
    local_path: &std::path::Path,
    parent_file_id: &str,
    file_name: &str,
    file_size: i64,
    pb: Option<ProgressBar>,
) -> Result<UploadReceipt, ClientError> {
    if file_size < 0 {
        return Err(ClientError::InvalidFilePath);
    }
    let file_size = super::super::upload_service::checked_upload_size(local_path)?;
    let part_size = personal.upload_part_size(file_size);
    checked_part_count(file_size, part_size)?;
    pb_debug("开始计算文件 SHA-256", &pb);
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
        pb_debug("开始上传分片", &pb);
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
        pb_debug("服务端确认秒传或创建空文件", &pb);
    }
    Ok(receipt)
}

struct UploadPartsParams<'a> {
    personal: &'a api::PersonalClient<'a>,
    local_path: &'a std::path::Path,
    upload_id: &'a str,
    file_id: &'a str,
    file_size: i64,
    content_hash: &'a str,
    part_size: i64,
    initial_parts: &'a [crate::models::PersonalPartInfo],
    pb: Option<ProgressBar>,
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
    let data_client = api::HttpClientWrapper::new();
    if file.metadata().await?.len() < file_size as u64 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "本地文件在上传前被截断",
        )
        .into());
    }
    let part_count = checked_part_count(file_size, part_size)?;

    for batch_start in (0..part_count).step_by(PART_BATCH_SIZE as usize) {
        let batch_end = (batch_start + PART_BATCH_SIZE).min(part_count);
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
            pb_debug(
                &format!("正在上传分片 part={}/{}", part_number, part_count),
                &params.pb,
            );
            super::personal_parts::upload_part(
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

async fn install_uploaded_file(
    personal: &api::PersonalClient<'_>,
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
            .filter(|id| !id.is_empty() && *id != "/" && *id != file_id)
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
    personal: &api::PersonalClient<'_>,
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
        let client = api::HttpClientWrapper::new();
        let personal = api::PersonalClient::with_host(
            &config,
            server.base_url(),
            &client,
            api::ListingRetry::None,
        );
        for gib in [5_i64, 8, 50, 500] {
            let size = gib * 1024 * 1024 * 1024 + 1;
            let request = server.mock(|when, then| {
                when.method(POST).path("/file/create")
                    .header("x-yun-client-info", api::headers::PC_CLIENT_INFO)
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
                .header("x-yun-client-info", api::headers::PC_CLIENT_INFO);
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
        let message = error.to_string();
        assert!(message.contains("04010319") && message.contains("权益不足"));
        rejected.assert_calls(1);
    }

    #[tokio::test]
    async fn ordinary_and_silver_limits_are_enforced_by_server_not_local_policy() {
        let server = MockServer::start();
        let gib = 1024_i64 * 1024 * 1024;
        let client = api::HttpClientWrapper::new();
        for (plan, limit) in [("ordinary", 5 * gib), ("silver", 8 * gib)] {
            let config = crate::config::Config {
                authorization: format!("fictional-{plan}-token"),
                ..Default::default()
            };
            let personal = api::PersonalClient::with_host(
                &config,
                server.base_url(),
                &client,
                api::ListingRetry::None,
            );
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
                        .header("x-yun-client-info", api::headers::PC_CLIENT_INFO)
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
            .map(|part_number| crate::models::PersonalPartInfo {
                part_number,
                upload_url: server.url("/initial"),
            })
            .collect();
        let client = api::HttpClientWrapper::new();
        let personal = api::PersonalClient::with_host(
            &config,
            server.base_url(),
            &client,
            api::ListingRetry::None,
        );
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
        let client = api::HttpClientWrapper::new();
        let personal = api::PersonalClient::with_host(
            &config,
            server.base_url(),
            &client,
            api::ListingRetry::None,
        );
        let error = upload_parts(UploadPartsParams {
            personal: &personal,
            local_path: source.path(),
            upload_id: "upload",
            file_id: "new",
            file_size: 2,
            content_hash: "fictional-hash",
            part_size: 2,
            initial_parts: &[crate::models::PersonalPartInfo {
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
        let client = api::HttpClientWrapper::new();
        let personal = api::PersonalClient::with_host(
            &config,
            server.base_url(),
            &client,
            api::ListingRetry::None,
        );
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
            let client = api::HttpClientWrapper::new();
            let personal = api::PersonalClient::with_host(
                &config,
                server.base_url(),
                &client,
                api::ListingRetry::None,
            );
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
