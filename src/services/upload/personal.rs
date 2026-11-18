use crate::client::{ClientError, api};
use crate::domain::StorageType;
use crate::models::{PersonalFileItem, PersonalUploadResp};
use crate::services::sync_service::{SYNC_UPLOAD_PREFIX, UPLOAD_BACKUP_PREFIX};
use crate::utils::logger::{pb_debug, pb_error, pb_success, pb_warn};
use indicatif::ProgressBar;
use std::io::{Read, Seek};

pub async fn upload(
    config: &crate::config::Config,
    local_path: &std::path::Path,
    remote_path: &str,
    file_name: &str,
    file_size: i64,
    force: bool,
    pb: Option<ProgressBar>,
) -> Result<(), ClientError> {
    if file_size < 0 {
        return Err(ClientError::InvalidFilePath);
    }
    let file_size = super::super::upload_service::checked_upload_size(local_path)?;
    let mut config = config.clone();
    let host = crate::client::api::get_personal_cloud_host(&mut config).await?;
    let parent_file_id = if remote_path == "/" || remote_path.is_empty() {
        "/".to_string()
    } else {
        crate::client::api::get_file_id_by_path(&config, remote_path).await?
    };

    if !force {
        check_file_exists(&config, &parent_file_id, file_name, &pb).await?;
    }

    let saved_name = upload_to_parent(
        &config,
        &host,
        local_path,
        &parent_file_id,
        file_name,
        file_size,
        pb.clone(),
    )
    .await?;
    finish_upload(&saved_name, &pb);
    Ok(())
}

fn finish_upload(name: &str, pb: &Option<ProgressBar>) {
    pb_success(&format!("上传完成: {name}"), pb);
    if let Some(bar) = pb {
        bar.finish_and_clear();
    }
}

/// Upload a unique sibling first, then promote it. Server-side entitlement,
/// transfer, or completion failures must not delete the existing destination.
pub(in crate::services) async fn upload_for_sync(
    config: &crate::config::Config,
    local_path: &std::path::Path,
    remote_path: &str,
    file_name: &str,
    replace_directory_id: Option<&str>,
    pb: Option<ProgressBar>,
) -> Result<(), ClientError> {
    let file_size = super::super::upload_service::checked_upload_size(local_path)?;
    super::super::directory_service::validate_name(file_name)?;
    let mut config = config.clone();
    let host = api::get_personal_cloud_host(&mut config).await?;
    let parent_id = if remote_path == "/" || remote_path.is_empty() {
        "/".to_owned()
    } else {
        api::get_file_id_by_path(&config, remote_path).await?
    };
    let temporary_name = format!(
        "{SYNC_UPLOAD_PREFIX}{}",
        crate::utils::crypto::generate_random_string(24)
    );
    let before = api::list_personal_files(&config, &parent_id).await?;
    if before
        .iter()
        .any(|item| item.name.as_deref() == Some(&temporary_name))
    {
        return Err(ClientError::Api("临时上传名称冲突，旧条目未替换".into()));
    }
    let uploaded = transfer_to_parent(
        &config,
        &host,
        local_path,
        &parent_id,
        &temporary_name,
        file_size,
        pb.clone(),
    )
    .await
    .map_err(|error| {
        ClientError::Api(format!(
            "同步上传 {file_name} 失败，旧条目未替换；临时名称 {temporary_name}: {error}"
        ))
    })?;
    let saved_name = uploaded.name.as_deref().unwrap_or(&temporary_name);
    let promote = async {
        let id = uploaded
            .id
            .as_deref()
            .ok_or_else(|| ClientError::Api("服务端未返回临时文件 ID".into()))?;
        if before
            .iter()
            .any(|item| item.file_id.as_deref() == Some(id))
        {
            return Err(ClientError::Api("临时文件 ID 已存在，旧条目未替换".into()));
        }
        let files = confirm_uploaded_name(&config, &parent_id, id, saved_name).await?;
        install_uploaded_file(
            &config,
            &host,
            &parent_id,
            id,
            file_name,
            &files,
            replace_directory_id,
        )
        .await
    }
    .await
    .map_err(|error| {
        ClientError::Api(format!(
            "临时名称 {saved_name}，替换 {file_name} 未完成: {error}"
        ))
    })?;
    finish_upload(&promote, &pb);
    Ok(())
}

pub(in crate::services) async fn upload_to_parent(
    config: &crate::config::Config,
    host: &str,
    local_path: &std::path::Path,
    parent_file_id: &str,
    file_name: &str,
    file_size: i64,
    pb: Option<ProgressBar>,
) -> Result<String, ClientError> {
    let uploaded = transfer_to_parent(
        config,
        host,
        local_path,
        parent_file_id,
        file_name,
        file_size,
        pb,
    )
    .await?;
    if uploaded.multipart {
        let id = uploaded.id.as_deref().ok_or(ClientError::InvalidFilePath)?;
        return handle_name_conflict(
            config,
            host,
            parent_file_id,
            id,
            file_name,
            uploaded.name.as_deref(),
        )
        .await;
    }
    Ok(uploaded.name.unwrap_or_else(|| file_name.to_owned()))
}

#[derive(Default)]
struct UploadReceipt {
    name: Option<String>,
    id: Option<String>,
    multipart: bool,
}

async fn transfer_to_parent(
    config: &crate::config::Config,
    host: &str,
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
    checked_part_count(
        file_size,
        crate::services::upload_service::get_part_size(file_size, config.custom_upload_part_size),
    )?;
    pb_debug("计算文件哈希...", &pb);
    let local_path_str = local_path.to_str().ok_or(ClientError::InvalidSourcePath)?;
    let content_hash = crate::utils::crypto::calc_file_sha256(local_path_str)?;
    let response = init_upload(
        config,
        &format!("{host}/file/create"),
        local_path,
        file_name,
        file_size,
        parent_file_id,
        &content_hash,
    )
    .await?;
    let Some(data) = response.data else {
        return Ok(UploadReceipt::default());
    };
    if data.exist.unwrap_or(false) {
        pb_debug("服务器复用已有文件内容", &pb);
    }
    let file_id = data.file_id.filter(|id| !id.is_empty() && id != "/");
    let Some(parts) = data.part_infos.as_deref().filter(|parts| !parts.is_empty()) else {
        return Ok(UploadReceipt {
            name: data.file_name,
            id: file_id,
            multipart: false,
        });
    };
    let file_id = file_id.ok_or(ClientError::InvalidFilePath)?;
    let upload_id = data
        .upload_id
        .filter(|id| !id.is_empty())
        .ok_or_else(|| ClientError::Api("服务端未返回分片上传 ID".into()))?;
    pb_debug("开始分片上传...", &pb);
    upload_parts(UploadPartsParams {
        config,
        host,
        local_path,
        upload_id: &upload_id,
        file_id: &file_id,
        file_size,
        content_hash: &content_hash,
        part_size: crate::services::upload_service::get_part_size(
            file_size,
            config.custom_upload_part_size,
        ),
        initial_parts: parts,
        pb,
    })
    .await?;
    Ok(UploadReceipt {
        name: data.file_name,
        id: Some(file_id),
        multipart: true,
    })
}

async fn check_file_exists(
    config: &crate::config::Config,
    parent_file_id: &str,
    file_name: &str,
    pb: &Option<ProgressBar>,
) -> Result<(), ClientError> {
    let exists = crate::client::api::check_file_exists(config, parent_file_id, file_name).await?;
    if exists {
        pb_warn(
            &format!("云端已存在「{}」，如果继续则云端会自动覆盖", file_name),
            pb,
        );
        pb_error("请使用 --force 参数确认继续", pb);
        return Err(ClientError::ForceRequired);
    }
    Ok(())
}

fn checked_part_count(file_size: i64, part_size: i64) -> Result<i64, ClientError> {
    if file_size < 0 || part_size <= 0 {
        return Err(ClientError::Api(
            "文件大小必须非负，上传分片大小必须大于 0".into(),
        ));
    }
    let count = file_size / part_size + i64::from(file_size % part_size != 0);
    if count > i32::MAX as i64 {
        return Err(ClientError::Api("上传分片数量超出协议范围".into()));
    }
    Ok(count)
}

/// Initialize the personal-cloud upload protocol without transferring content.
/// Size and SHA256 may be supplied by callers that already have this metadata.
pub async fn init_upload(
    config: &crate::config::Config,
    url: &str,
    local_path: &std::path::Path,
    file_name: &str,
    file_size: i64,
    parent_file_id: &str,
    content_hash: &str,
) -> Result<PersonalUploadResp, ClientError> {
    let part_size =
        crate::services::upload_service::get_part_size(file_size, config.custom_upload_part_size);
    let part_count = checked_part_count(file_size, part_size)?;

    let first_part_infos: Vec<serde_json::Value> = (0..part_count.min(100))
        .map(|i| {
            let start = i * part_size;
            let byte_size = if file_size - start > part_size {
                part_size
            } else {
                file_size - start
            };
            serde_json::json!({
                "partNumber": (i + 1) as i32,
                "partSize": byte_size,
                "parallelHashCtx": {
                    "partOffset": start
                }
            })
        })
        .collect();

    let content_type = get_content_type(local_path);

    let body = serde_json::json!({
        "contentHash": content_hash,
        "contentHashAlgorithm": "SHA256",
        "contentType": content_type,
        "parallelUpload": false,
        "partInfos": first_part_infos,
        "size": file_size,
        "parentFileId": parent_file_id,
        "name": file_name,
        "type": "file",
        "fileRenameMode": "auto_rename"
    });

    let resp: PersonalUploadResp =
        api::personal_api_request(config, url, body, StorageType::PersonalNew).await?;

    if !resp.base.success {
        let quota_hint = if resp.base.code.as_deref() == Some("04010319") {
            format!("；文件大小 {file_size} 字节，请核对账号当前的单文件上传额度")
        } else {
            String::new()
        };
        return Err(ClientError::Api(format!(
            "创建上传任务失败: {}{}{}",
            resp.base.message.as_deref().unwrap_or("未知错误"),
            resp.base
                .code
                .as_deref()
                .map(|code| format!("（错误码: {code}）"))
                .unwrap_or_default(),
            quota_hint,
        )));
    }

    Ok(resp)
}

fn get_content_type(path: &std::path::Path) -> String {
    match path.extension().and_then(|e| e.to_str()) {
        Some("txt") => "text/plain",
        Some("html") | Some("htm") => "text/html",
        Some("css") => "text/css",
        Some("js") => "application/javascript",
        Some("json") => "application/json",
        Some("xml") => "application/xml",
        Some("pdf") => "application/pdf",
        Some("zip") => "application/zip",
        Some("tar") => "application/x-tar",
        Some("gz") => "application/gzip",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("bmp") => "image/bmp",
        Some("svg") => "image/svg+xml",
        Some("mp3") => "audio/mpeg",
        Some("mp4") => "video/mp4",
        Some("avi") => "video/x-msvideo",
        Some("mov") => "video/quicktime",
        _ => "application/octet-stream",
    }
    .to_string()
}

pub(super) struct UploadPartsParams<'a> {
    config: &'a crate::config::Config,
    host: &'a str,
    local_path: &'a std::path::Path,
    upload_id: &'a str,
    file_id: &'a str,
    file_size: i64,
    content_hash: &'a str,
    part_size: i64,
    initial_parts: &'a [crate::models::PersonalPartInfo],
    pb: Option<ProgressBar>,
}

pub(super) async fn upload_parts(params: UploadPartsParams<'_>) -> Result<(), ClientError> {
    let config = params.config;
    let host = params.host;
    let local_path = params.local_path;
    let upload_id = params.upload_id;
    let file_id = params.file_id;
    let file_size = params.file_size;
    let content_hash = params.content_hash;
    let part_size = params.part_size;

    let mut file = std::fs::File::open(local_path)?;
    let part_count = checked_part_count(file_size, part_size)?;

    for batch_start in (0..part_count).step_by(100) {
        let batch_end = (batch_start + 100).min(part_count);
        let upload_urls = if batch_start == 0 {
            params
                .initial_parts
                .iter()
                .map(|part| (part.part_number, part.upload_url.clone()))
                .collect()
        } else {
            super::personal_parts::get_upload_url_batch(
                config,
                host,
                file_id,
                upload_id,
                batch_start..batch_end,
                part_size,
                file_size,
            )
            .await?
        };
        for i in batch_start..batch_end {
            file.seek(std::io::SeekFrom::Start(i as u64 * part_size as u64))?;

            let read_size = part_size.min(file_size - i * part_size);
            let mut buffer = vec![0u8; read_size as usize];
            file.read_exact(&mut buffer)?;

            let part_number = (i + 1) as i32;
            pb_debug(
                &format!("上传分片 {}/{}", part_number, part_count),
                &params.pb,
            );

            crate::services::upload::personal_parts::upload_single_part(
                &upload_urls,
                part_number,
                &buffer,
                params.pb.clone(),
            )
            .await?;
        }
    }

    crate::services::upload::personal_parts::confirm_upload(
        config,
        host,
        file_id,
        upload_id,
        content_hash,
    )
    .await
}

async fn handle_name_conflict(
    config: &crate::config::Config,
    host: &str,
    parent_file_id: &str,
    file_id: &str,
    file_name: &str,
    reported_name: Option<&str>,
) -> Result<String, ClientError> {
    if reported_name == Some(file_name) {
        return Ok(file_name.to_owned());
    }
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    let files = api::list_personal_files(config, parent_file_id).await?;
    install_uploaded_file(
        config,
        host,
        parent_file_id,
        file_id,
        file_name,
        &files,
        None,
    )
    .await
}

async fn install_uploaded_file(
    config: &crate::config::Config,
    host: &str,
    parent_file_id: &str,
    file_id: &str,
    file_name: &str,
    files: &[PersonalFileItem],
    replace_directory_id: Option<&str>,
) -> Result<String, ClientError> {
    let saved_name = files
        .iter()
        .find(|item| item.file_id.as_deref() == Some(file_id) && !item.is_folder())
        .and_then(|item| item.name.as_deref())
        .filter(|name| !name.is_empty())
        .ok_or_else(|| ClientError::Api("分片上传已提交，但无法确认新文件名称".into()))?;
    if saved_name == file_name {
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
            crate::utils::crypto::generate_random_string(20)
        );
        rename_uploaded_file(config, host, old_id, &backup_name)
            .await
            .map_err(|error| {
                ClientError::Api(format!(
                    "无法为旧文件保留备份，新文件保存在 {saved_name}: {error}"
                ))
            })?;
        rename_uploaded_file(config, host, file_id, file_name).await.map_err(|error| {
            ClientError::Api(format!("无法重命名新文件；旧文件保留为 {backup_name}，请检查新文件 {saved_name}: {error}"))
        })?;
        confirm_uploaded_name(config, parent_file_id, file_id, file_name)
            .await
            .map_err(|error| {
                ClientError::Api(format!(
                    "无法确认新文件名称；旧文件备份 {backup_name} 未删除: {error}"
                ))
            })?;
        super::super::delete_service::trash_personal_file_id(config, host, old_id)
            .await
            .map_err(|error| ClientError::Api(format!(
                "新文件已重命名为 {file_name}，但旧文件备份 {backup_name} 未确认移到回收站: {error}"
            )))?;
    } else {
        rename_uploaded_file(config, host, file_id, file_name).await?;
        confirm_uploaded_name(config, parent_file_id, file_id, file_name).await?;
    }
    Ok(file_name.to_owned())
}

async fn confirm_uploaded_name(
    config: &crate::config::Config,
    parent_id: &str,
    file_id: &str,
    name: &str,
) -> Result<Vec<PersonalFileItem>, ClientError> {
    for attempt in 0..5 {
        let files = api::list_personal_files(config, parent_id).await?;
        let mut matches = files
            .iter()
            .filter(|item| item.name.as_deref() == Some(name));
        if matches
            .next()
            .is_some_and(|item| item.file_id.as_deref() == Some(file_id) && !item.is_folder())
            && matches.next().is_none()
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

async fn rename_uploaded_file(
    config: &crate::config::Config,
    host: &str,
    file_id: &str,
    name: &str,
) -> Result<(), ClientError> {
    let response: PersonalUploadResp = api::personal_api_request(
        config,
        &format!("{host}/file/update"),
        serde_json::json!({ "fileId": file_id, "name": name, "description": "" }),
        StorageType::PersonalNew,
    )
    .await?;
    if !response.base.success {
        return Err(ClientError::Api(
            response.base.message.unwrap_or_else(|| "重命名失败".into()),
        ));
    }
    Ok(())
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
        for gib in [5_i64, 8, 50, 500] {
            let size = gib * 1024 * 1024 * 1024 + 1;
            let request = server.mock(|when, then| {
                when.method(POST).path("/file/create")
                    .header("x-yun-client-info", crate::client::PC_CLIENT_INFO)
                    .json_body_includes(format!("{{\"size\":{size}}}"));
                then.json_body(json!({"success":true,"data":{"fileId":"new","uploadId":"upload","partInfos":[{"partNumber":1,"uploadUrl":"https://example.invalid/part"}]}}));
            });
            let response = init_upload(
                &config,
                &server.url("/file/create"),
                std::path::Path::new("large.bin"),
                "large.bin",
                size,
                "/",
                "fictional-hash",
            )
            .await
            .unwrap();
            assert_eq!(response.data.unwrap().file_id.as_deref(), Some("new"));
            request.assert_calls(1);
        }
        let rejected = server.mock(|when, then| {
            when.method(POST)
                .path("/denied")
                .header("x-yun-client-info", crate::client::PC_CLIENT_INFO);
            then.json_body(json!({"success":false,"code":"04010319","message":"权益不足"}));
        });
        let error = init_upload(
            &config,
            &server.url("/denied"),
            std::path::Path::new("small.bin"),
            "small.bin",
            1,
            "/",
            "fictional-hash",
        )
        .await
        .unwrap_err();
        let message = error.to_string();
        assert!(message.contains("04010319") && message.contains("权益不足"));
        rejected.assert_calls(1);
    }

    #[tokio::test]
    async fn ordinary_and_silver_limits_are_enforced_by_server_not_local_policy() {
        let server = MockServer::start();
        let gib = 1024_i64 * 1024 * 1024;
        for (plan, limit) in [("ordinary", 5 * gib), ("silver", 8 * gib)] {
            let config = crate::config::Config {
                authorization: format!("fictional-{plan}-token"),
                ..Default::default()
            };
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
                        .header("x-yun-client-info", crate::client::PC_CLIENT_INFO)
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
                let result = init_upload(
                    &config,
                    &server.url("/file/create"),
                    std::path::Path::new("boundary.bin"),
                    "boundary.bin",
                    size,
                    "/",
                    "fictional-hash",
                )
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
        upload_parts(UploadPartsParams {
            config: &config,
            host: &server.base_url(),
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
        let error = upload_parts(UploadPartsParams {
            config: &config,
            host: &server.base_url(),
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
        let error = handle_name_conflict(
            &config,
            &server.base_url(),
            "/",
            "new",
            "report.txt",
            Some("temporary.txt"),
        )
        .await
        .unwrap_err();
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
            let result = handle_name_conflict(
                &config,
                &server.base_url(),
                "/",
                "new",
                "report.txt",
                Some("report-copy.txt"),
            )
            .await;
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
