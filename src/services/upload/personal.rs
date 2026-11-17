use crate::client::{ClientError, api};
use crate::domain::StorageType;
use crate::models::PersonalUploadResp;
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

    upload_to_parent(
        &config,
        &host,
        local_path,
        &parent_file_id,
        file_name,
        file_size,
        pb,
    )
    .await
}

pub(in crate::services) async fn upload_to_parent(
    config: &crate::config::Config,
    host: &str,
    local_path: &std::path::Path,
    parent_file_id: &str,
    file_name: &str,
    file_size: i64,
    pb: Option<ProgressBar>,
) -> Result<(), ClientError> {
    pb_debug("计算文件哈希...", &pb);
    let local_path_str = local_path.to_str().ok_or(ClientError::InvalidSourcePath)?;
    let content_hash = crate::utils::crypto::calc_file_sha256(local_path_str)?;

    let init_resp = init_upload(
        config,
        &format!("{host}/file/create"),
        local_path,
        file_name,
        file_size,
        parent_file_id,
        &content_hash,
    )
    .await?;

    if init_resp.data.is_none() {
        pb_success(&format!("上传完成: {}", file_name), &pb);
        if let Some(ref pb_bar) = pb {
            pb_bar.finish_and_clear();
        }
        return Ok(());
    }

    let data = init_resp.data.unwrap();

    if data.exist.unwrap_or(false) {
        pb_warn(
            &format!("文件已存在: {}", data.file_name.as_deref().unwrap_or("")),
            &pb,
        );
    }

    if let Some(part_infos_response) = data.part_infos {
        if part_infos_response.is_empty() {
            pb_debug("服务器未返回分片信息", &pb);
            pb_success(
                &format!(
                    "上传完成: {}",
                    data.file_name.unwrap_or_else(|| file_name.to_string())
                ),
                &pb,
            );
            if let Some(ref pb_bar) = pb {
                pb_bar.finish_and_clear();
            }
        } else {
            let file_id = data
                .file_id
                .filter(|id| !id.is_empty() && id != "/")
                .ok_or(ClientError::InvalidFilePath)?;
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
                pb: pb.clone(),
            })
            .await?;

            let saved_name = handle_name_conflict(
                config,
                host,
                parent_file_id,
                &file_id,
                file_name,
                data.file_name.as_deref(),
            )
            .await?;

            pb_success(&format!("上传完成: {saved_name}"), &pb);
            if let Some(ref pb_bar) = pb {
                pb_bar.finish_and_clear();
            }
        }
    } else {
        pb_debug("服务器未返回分片信息", &pb);
        pb_success(&format!("上传完成: {}", file_name), &pb);
        if let Some(ref pb_bar) = pb {
            pb_bar.finish_and_clear();
        }
    }

    Ok(())
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
        crate::services::upload_service::get_part_size(file_size, config.custom_upload_part_size);
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
        api::personal_api_request(config, url, body, StorageType::PersonalNew).await?;

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

    let upload_urls = crate::services::upload::personal_parts::get_upload_urls(
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
        pb_debug(
            &format!("上传分片 {}/{}", part_number, part_count),
            &params.pb,
        );

        crate::services::upload::personal_parts::upload_single_part(
            &upload_urls,
            part_number,
            &buffer[..bytes_read],
            params.pb.clone(),
        )
        .await?;
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
    let saved_name = files
        .iter()
        .find(|item| item.file_id.as_deref() == Some(file_id))
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
        if conflicts.next().is_some() || item.is_folder() {
            return Err(ClientError::Api(format!(
                "同名条目无法安全替换，新文件保存在 {saved_name}"
            )));
        }
        let old_id = item
            .file_id
            .as_deref()
            .filter(|id| !id.is_empty() && *id != "/" && *id != file_id)
            .ok_or(ClientError::InvalidFilePath)?;
        let backup_name = format!(
            ".cloud139-backup-{}",
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
        super::super::delete_service::trash_personal_file_id(config, host, old_id)
            .await
            .map_err(|error| ClientError::Api(format!(
                "新文件已重命名为 {file_name}，但旧文件备份 {backup_name} 未确认移到回收站: {error}"
            )))?;
    } else {
        rename_uploaded_file(config, host, file_id, file_name).await?;
    }
    Ok(file_name.to_owned())
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
