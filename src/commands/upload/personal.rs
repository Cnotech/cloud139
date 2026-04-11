use crate::client::{ClientError, StorageType};
use crate::models::PersonalUploadResp;
use crate::{error, info, step, success, warn};
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
    let mut config = config.clone();
    let host = crate::client::api::get_personal_cloud_host(&mut config).await?;
    let url = format!("{}/file/create", host);

    let (parent_file_id, content_hash) =
        prepare_upload(config.clone(), local_path, remote_path).await?;

    if !force {
        check_file_exists(&config, &parent_file_id, file_name).await?;
    }

    let init_resp = init_upload(
        &config,
        &url,
        local_path,
        file_name,
        file_size,
        &parent_file_id,
        &content_hash,
    )
    .await?;

    if init_resp.data.is_none() {
        if let Some(pb) = pb {
            pb.finish_and_clear();
        }
        success!("上传完成: {}", file_name);
        return Ok(());
    }

    let data = init_resp.data.unwrap();

    if data.exist.unwrap_or(false) {
        warn!("文件已存在: {}", data.file_name.as_deref().unwrap_or(""));
    }

    if let Some(part_infos_response) = data.part_infos {
        if part_infos_response.is_empty() {
            warn!("服务器未返回分片信息");
            if let Some(pb) = &pb {
                pb.finish_and_clear();
            }
            success!(
                "上传完成: {}",
                data.file_name.unwrap_or_else(|| file_name.to_string())
            );
        } else {
            let file_id_val = data.file_id.clone().unwrap_or_default();
            let file_name_val = data.file_name.clone();

            step!("开始分片上传...");
            upload_parts(UploadPartsParams {
                config: &config,
                host: &host,
                local_path,
                upload_id: &data.upload_id.unwrap_or_default(),
                file_id: &file_id_val,
                file_size,
                content_hash: &content_hash,
                part_size: crate::commands::upload::get_part_size(
                    file_size,
                    config.custom_upload_part_size,
                ),
                pb: pb.clone(),
            })
            .await?;

            handle_name_conflict(
                &config,
                &host,
                &parent_file_id,
                &file_id_val,
                file_name,
                &file_name_val,
            )
            .await?;

            if let Some(pb) = &pb {
                pb.finish_and_clear();
            }
            success!("上传完成: {}", file_name_val.as_deref().unwrap_or(""));
        }
    } else {
        warn!("服务器未返回分片信息");
        if let Some(pb) = &pb {
            pb.finish_and_clear();
        }
        success!("上传完成: {}", file_name);
    }

    Ok(())
}

async fn prepare_upload(
    config: crate::config::Config,
    local_path: &std::path::Path,
    remote_path: &str,
) -> Result<(String, String), ClientError> {
    info!("计算文件哈希...");
    let local_path_str = local_path
        .to_str()
        .ok_or_else(|| ClientError::InvalidSourcePath)?;
    let content_hash = crate::utils::crypto::calc_file_sha256(local_path_str)?;

    let parent_file_id = if remote_path == "/" || remote_path.is_empty() {
        "/".to_string()
    } else {
        crate::client::api::get_file_id_by_path(&config, remote_path).await?
    };

    Ok((parent_file_id, content_hash))
}

async fn check_file_exists(
    config: &crate::config::Config,
    parent_file_id: &str,
    file_name: &str,
) -> Result<(), ClientError> {
    let exists = crate::client::api::check_file_exists(config, parent_file_id, file_name).await?;
    if exists {
        warn!("云端已存在「{}」，如果继续则云端会自动覆盖", file_name);
        error!("请使用 --force 参数确认继续");
        return Err(ClientError::ForceRequired);
    }
    Ok(())
}

async fn init_upload(
    config: &crate::config::Config,
    url: &str,
    local_path: &std::path::Path,
    file_name: &str,
    file_size: i64,
    parent_file_id: &str,
    content_hash: &str,
) -> Result<PersonalUploadResp, ClientError> {
    let part_size =
        crate::commands::upload::get_part_size(file_size, config.custom_upload_part_size);
    let part_count = (file_size + part_size - 1) / part_size;

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
        crate::client::api::personal_api_request(config, url, body, StorageType::PersonalNew)
            .await?;

    if !resp.base.success {
        return Err(ClientError::Api(format!(
            "创建上传任务失败: {}",
            resp.base.message.as_deref().unwrap_or("未知错误")
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
    let part_count = (file_size + part_size - 1) / part_size;

    let upload_urls = super::personal_parts::get_upload_urls(
        config, host, file_id, upload_id, part_count, part_size, file_size,
    )
    .await?;

    for i in 0..part_count {
        file.seek(std::io::SeekFrom::Start(i as u64 * part_size as u64))?;

        let read_size = if (i + 1) * part_size > file_size {
            file_size - i * part_size
        } else {
            part_size
        };

        let mut buffer = vec![0u8; read_size as usize];
        let bytes_read = Read::read(&mut file, &mut buffer)?;

        if bytes_read == 0 {
            break;
        }

        let part_number = (i + 1) as i32;
        step!("上传分片 {}/{}", part_number, part_count);

        super::personal_parts::upload_single_part(&upload_urls, part_number, &buffer[..bytes_read])
            .await?;

        if let Some(pb) = &params.pb {
            pb.inc(bytes_read as u64);
        }
    }

    super::personal_parts::confirm_upload(config, host, file_id, upload_id, content_hash).await
}

async fn handle_name_conflict(
    config: &crate::config::Config,
    host: &str,
    parent_file_id: &str,
    file_id_val: &str,
    file_name: &str,
    file_name_val: &Option<String>,
) -> Result<(), ClientError> {
    if file_name_val.as_deref() != Some(file_name) {
        warn!(
            "检测到文件名冲突: {} != {}",
            file_name_val.as_deref().unwrap_or(""),
            file_name
        );
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        let files = crate::client::api::list_personal_files(config, parent_file_id).await?;
        for file in &files {
            #[allow(clippy::needless_borrow)]
            if file.name.as_deref() == Some(&file_name) {
                step!("冲突处理: 先重命名旧文件避免冲突");
                let old_name = format!(
                    "{}_{}",
                    file_name,
                    crate::utils::crypto::generate_random_string(4)
                );
                let rename_old_url = format!("{}/file/update", host);
                let rename_old_body = serde_json::json!({
                    "fileId": file.file_id.as_ref().unwrap_or(&String::new()),
                    "name": old_name,
                    "description": ""
                });
                let _: PersonalUploadResp = crate::client::api::personal_api_request(
                    config,
                    &rename_old_url,
                    rename_old_body,
                    StorageType::PersonalNew,
                )
                .await?;
                step!("冲突处理: 删除旧文件");
                let del_url = format!("{}/recyclebin/batchTrash", host);
                let del_body = serde_json::json!({
                    "fileIds": [file.file_id.as_ref().unwrap_or(&String::new())]
                });
                let _: serde_json::Value = crate::client::api::personal_api_request(
                    config,
                    &del_url,
                    del_body,
                    StorageType::PersonalNew,
                )
                .await?;
                break;
            }
        }

        for file in &files {
            if file.file_id.as_deref() == Some(file_id_val) {
                step!("冲突处理: 重命名新文件");
                let rename_url = format!("{}/file/update", host);
                let rename_body = serde_json::json!({
                    "fileId": file_id_val,
                    "name": file_name,
                    "description": ""
                });
                let _: PersonalUploadResp = crate::client::api::personal_api_request(
                    config,
                    &rename_url,
                    rename_body,
                    StorageType::PersonalNew,
                )
                .await?;
                break;
            }
        }
    }
    Ok(())
}
