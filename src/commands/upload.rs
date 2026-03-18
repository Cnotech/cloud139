use crate::client::{ClientError, StorageType};
use crate::models::PersonalUploadResp;
use crate::{error, info, step, success, warn};
use clap::Parser;
use std::path::Path;

#[derive(Parser, Debug)]
pub struct UploadArgs {
    #[arg(help = "本地文件路径")]
    pub local_path: String,

    #[arg(default_value = "/", help = "远程目录路径")]
    pub remote_path: String,

    #[arg(short, long, help = "强制继续，如果云端存在同名文件则自动重命名")]
    pub force: bool,
}

pub async fn execute(args: UploadArgs) -> Result<(), ClientError> {
    let config = crate::config::Config::load().map_err(ClientError::Config)?;
    let storage_type = config.storage_type();

    let local_path = Path::new(&args.local_path);
    if !local_path.exists() {
        error!("文件不存在: {}", args.local_path);
        return Err(ClientError::FileNotFound);
    }

    let file_name = local_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    let metadata = std::fs::metadata(local_path)?;
    let file_size = metadata.len() as i64;

    let remote_dir = if args.remote_path == "/" {
        "".to_string()
    } else {
        args.remote_path.trim_end_matches('/').to_string()
    };

    info!(
        "上传文件: {} -> {}/{}",
        args.local_path, remote_dir, file_name
    );
    info!("文件大小: {} bytes", file_size);

    match storage_type {
        StorageType::PersonalNew => {
            upload_personal(
                &config,
                local_path,
                &remote_dir,
                file_name,
                file_size,
                args.force,
            )
            .await?;
        }
        StorageType::Family => {
            upload_family(&config, local_path, &remote_dir, file_name, file_size).await?;
        }
        StorageType::Group => {
            upload_group(&config, local_path, &remote_dir, file_name, file_size).await?;
        }
    }

    Ok(())
}

pub fn get_part_size(size: i64, custom_size: i64) -> i64 {
    if custom_size != 0 {
        return custom_size;
    }
    if size / (1024 * 1024 * 1024) > 30 {
        return 512 * 1024 * 1024;
    }
    100 * 1024 * 1024
}

async fn upload_personal(
    config: &crate::config::Config,
    local_path: &Path,
    remote_path: &str,
    file_name: &str,
    file_size: i64,
    force: bool,
) -> Result<(), ClientError> {
    let mut config = config.clone();
    let host = crate::client::api::get_personal_cloud_host(&mut config).await?;
    let url = format!("{}/file/create", host);

    info!("计算文件哈希...");
    let content_hash = crate::utils::crypto::calc_file_sha256(local_path.to_str().unwrap())?;

    let parent_file_id = if remote_path == "/" || remote_path.is_empty() {
        "/".to_string()
    } else {
        crate::client::api::get_file_id_by_path(&config, remote_path).await?
    };

    if !force {
        let exists =
            crate::client::api::check_file_exists(&config, &parent_file_id, file_name).await?;
        if exists {
            warn!("云端已存在「{}」，如果继续则云端会自动覆盖", file_name);
            error!("请使用 --force 参数确认继续");
            return Err(ClientError::ForceRequired);
        }
    }

    let part_size = get_part_size(file_size, config.custom_upload_part_size);
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

    let content_type = match local_path.extension().and_then(|e| e.to_str()) {
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
    .to_string();

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
        crate::client::api::personal_api_request(&config, &url, body, StorageType::PersonalNew)
            .await?;

    if !resp.base.success {
        return Err(ClientError::Api(format!(
            "创建上传任务失败: {}",
            resp.base.message.as_deref().unwrap_or("未知错误")
        )));
    }

    let data = match resp.data {
        Some(d) => d,
        None => {
            success!("上传完成: {}", file_name);
            return Ok(());
        }
    };

    if data.exist.unwrap_or(false) {
        warn!("文件已存在: {}", data.file_name.as_deref().unwrap_or(""));
        // return Ok(());
    }

    if let Some(part_infos_response) = data.part_infos {
        if part_infos_response.is_empty() {
            warn!("服务器未返回分片信息");
            let file_name_val = data
                .file_name
                .clone()
                .unwrap_or_else(|| file_name.to_string());
            success!("上传完成: {}", file_name_val);
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
                part_size,
            })
            .await?;

            if file_name_val.as_deref() != Some(&file_name) {
                warn!(
                    "检测到文件名冲突: {} != {}",
                    file_name_val.as_deref().unwrap_or(""),
                    file_name
                );
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

                let files =
                    crate::client::api::list_personal_files(&config, &parent_file_id).await?;
                for file in &files {
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
                            &config,
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
                            &config,
                            &del_url,
                            del_body,
                            StorageType::PersonalNew,
                        )
                        .await?;
                        break;
                    }
                }

                for file in &files {
                    if file.file_id.as_ref() == Some(&file_id_val) {
                        step!("冲突处理: 重命名新文件");
                        let rename_url = format!("{}/file/update", host);
                        let rename_body = serde_json::json!({
                            "fileId": file_id_val,
                            "name": file_name,
                            "description": ""
                        });
                        let _: PersonalUploadResp = crate::client::api::personal_api_request(
                            &config,
                            &rename_url,
                            rename_body,
                            StorageType::PersonalNew,
                        )
                        .await?;
                        break;
                    }
                }
            }

            success!("上传完成: {}", file_name_val.as_deref().unwrap_or(""));
        }
    } else {
        warn!("服务器未返回分片信息");
        success!("上传完成: {}", file_name);
    }

    Ok(())
}

struct UploadPartsParams<'a> {
    config: &'a crate::config::Config,
    host: &'a str,
    local_path: &'a Path,
    upload_id: &'a str,
    file_id: &'a str,
    file_size: i64,
    content_hash: &'a str,
    part_size: i64,
}

async fn upload_parts(params: UploadPartsParams<'_>) -> Result<(), ClientError> {
    let config = params.config;
    let host = params.host;
    let local_path = params.local_path;
    let upload_id = params.upload_id;
    let file_id = params.file_id;
    let file_size = params.file_size;
    let content_hash = params.content_hash;
    let part_size = params.part_size;
    use std::collections::HashMap;
    use std::fs::File;
    use std::io::{Read, Seek, SeekFrom};

    let mut file = File::open(local_path)?;
    let part_count = (file_size + part_size - 1) / part_size;

    let mut upload_urls: HashMap<i32, String> = HashMap::new();

    for batch_start in (0..part_count as usize).step_by(100) {
        let batch_end = std::cmp::min(batch_start + 100, part_count as usize);

        let url = format!("{}/file/getUploadUrl", host);

        let part_infos: Vec<serde_json::Value> = (batch_start..batch_end)
            .map(|i| {
                let start = i as i64 * part_size;
                let byte_size = if file_size - start > part_size {
                    part_size
                } else {
                    file_size - start
                };
                serde_json::json!({
                    "partNumber": (i + 1) as i32,
                    "partSize": byte_size
                })
            })
            .collect();

        let body = serde_json::json!({
            "fileId": file_id,
            "uploadId": upload_id,
            "partInfos": part_infos,
            "commonAccountInfo": {
                "account": config.account,
                "accountType": 1
            }
        });

        let resp_json: serde_json::Value =
            crate::client::api::personal_api_request(config, &url, body, StorageType::PersonalNew)
                .await?;

        if let Some(part_infos) = resp_json
            .get("data")
            .and_then(|d| d.get("partInfos"))
            .and_then(|p| p.as_array())
        {
            for info in part_infos {
                if let (Some(part_num), Some(url)) = (
                    info.get("partNumber").and_then(|n| n.as_i64()),
                    info.get("uploadUrl").and_then(|u| u.as_str()),
                ) {
                    upload_urls.insert(part_num as i32, url.to_string());
                }
            }
        }
    }

    for i in 0..part_count {
        file.seek(SeekFrom::Start(i as u64 * part_size as u64))?;

        let read_size = if (i + 1) * part_size > file_size {
            file_size - i * part_size
        } else {
            part_size
        };

        let mut buffer = vec![0u8; read_size as usize];
        let bytes_read = file.read(&mut buffer)?;

        if bytes_read == 0 {
            break;
        }

        let part_number = (i + 1) as i32;
        step!("上传分片 {}/{}", part_number, part_count);

        let upload_url = upload_urls
            .get(&part_number)
            .cloned()
            .ok_or_else(|| ClientError::Api(format!("找不到分片 {} 的上传URL", part_number)))?;

        let resp_code = tokio::task::spawn_blocking(move || {
            ureq::put(&upload_url)
                .header("Content-Type", "application/octet-stream")
                .send(&buffer[..bytes_read])
                .map(|resp| resp.status().as_u16() as u32)
                .unwrap_or_else(|e| {
                    error!("上传失败: {}", e);
                    0
                })
        })
        .await
        .map_err(|e| ClientError::Api(format!("上传任务失败: {}", e)))?;

        if resp_code != 200 {
            return Err(ClientError::Api(format!("分片 {} 上传失败: {}", part_number, resp_code)));
        }
    }

    step!("\n所有分片上传完成");

    let complete_url = format!("{}/file/complete", host);

    let body = serde_json::json!({
        "contentHash": content_hash,
        "contentHashAlgorithm": "SHA256",
        "uploadId": upload_id,
        "fileId": file_id,
    });

    let resp_json: serde_json::Value = crate::client::api::personal_api_request(
        config,
        &complete_url,
        body,
        StorageType::PersonalNew,
    )
    .await?;

    if let Some(success_flag) = resp_json.get("success").and_then(|s| s.as_bool()) {
        if success_flag {
            info!("完成响应: {:?}", resp_json);
        } else {
            let message = resp_json
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("完成上传失败");
            return Err(ClientError::Api(format!("完成上传失败: {}", message)));
        }
    } else {
        info!("完成响应: {:?}", resp_json);
    }

    Ok(())
}

async fn upload_family(
    config: &crate::config::Config,
    local_path: &Path,
    remote_path: &str,
    file_name: &str,
    file_size: i64,
) -> Result<(), ClientError> {
    let client = crate::client::Client::new(config.clone());

    let url = "https://yun.139.com/orchestration/familyCloud-rebuild/content/v1.0/getFileUploadURL";

    let _parent_id = if remote_path == "/" || remote_path.is_empty() {
        config
            .root_folder_id
            .clone()
            .unwrap_or_else(|| "0".to_string())
    } else {
        remote_path.to_string()
    };

    let upload_path = if remote_path == "/" || remote_path.is_empty() {
        if let Some(ref root_path) = config.root_folder_id {
            root_path.clone()
        } else {
            "0".to_string()
        }
    } else {
        remote_path.to_string()
    };

    let report_size = if config.report_real_size {
        file_size
    } else {
        0
    };

    let body = serde_json::json!({
        "catalogType": 3,
        "cloudID": config.cloud_id,
        "cloudType": 1,
        "fileCount": 1,
        "manualRename": 2,
        "operation": 0,
        "path": upload_path,
        "seqNo": crate::utils::crypto::generate_random_string(32),
        "totalSize": report_size,
        "uploadContentList": [{
            "contentName": file_name,
            "contentSize": report_size
        }],
        "commonAccountInfo": {
            "account": config.account,
            "accountType": 1
        }
    });

    let resp: serde_json::Value = client.api_request_post(url, body).await?;

    if resp
        .get("result")
        .and_then(|r| r.get("resultCode"))
        .and_then(|c| c.as_str())
        != Some("0")
    {
        return Err(ClientError::Api(format!("获取上传URL失败: {:?}", resp)));
    }

    let upload_url = resp
        .pointer("/data/uploadResult/redirectionUrl")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ClientError::Api("未找到上传URL".to_string()))?;

    let upload_task_id = resp
        .pointer("/data/uploadResult/uploadTaskID")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ClientError::Api("未找到上传任务ID".to_string()))?;

    info!("开始上传文件到家庭云...");
    upload_file_to_url(local_path, upload_url, upload_task_id, file_size, file_name).await?;

    success!("上传完成!");
    Ok(())
}

async fn upload_group(
    config: &crate::config::Config,
    local_path: &Path,
    remote_path: &str,
    file_name: &str,
    file_size: i64,
) -> Result<(), ClientError> {
    let client = crate::client::Client::new(config.clone());

    let url = "https://yun.139.com/orchestration/group-rebuild/content/v1.0/getGroupFileUploadURL";

    let _parent_id = if remote_path == "/" || remote_path.is_empty() {
        "0".to_string()
    } else {
        remote_path.to_string()
    };

    let upload_path = if remote_path == "/" || remote_path.is_empty() {
        if let Some(ref root_path) = config.root_folder_id {
            root_path.clone()
        } else {
            "root:".to_string()
        }
    } else {
        format!("root:/{}", remote_path.trim_start_matches('/'))
    };

    let report_size = if config.report_real_size {
        file_size
    } else {
        0
    };

    let body = serde_json::json!({
        "fileCount": 1,
        "manualRename": 2,
        "operation": 0,
        "path": upload_path,
        "seqNo": crate::utils::crypto::generate_random_string(32),
        "totalSize": report_size,
        "uploadContentList": [{
            "contentName": file_name,
            "contentSize": report_size
        }],
        "groupID": config.cloud_id
    });

    let resp: serde_json::Value = client.api_request_post(url, body).await?;

    if resp
        .get("result")
        .and_then(|r| r.get("resultCode"))
        .and_then(|c| c.as_str())
        != Some("0")
    {
        return Err(ClientError::Api(format!("获取上传URL失败: {:?}", resp)));
    }

    let upload_url = resp
        .pointer("/data/uploadResult/redirectionUrl")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ClientError::Api("未找到上传URL".to_string()))?;

    let upload_task_id = resp
        .pointer("/data/uploadResult/uploadTaskID")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ClientError::Api("未找到上传任务ID".to_string()))?;

    info!("开始上传文件到群组云...");
    upload_file_to_url(local_path, upload_url, upload_task_id, file_size, file_name).await?;

    success!("上传完成!");
    Ok(())
}

async fn upload_file_to_url(
    local_path: &Path,
    upload_url: &str,
    upload_task_id: &str,
    file_size: i64,
    file_name: &str,
) -> Result<(), ClientError> {
    use std::io::{Read, Seek};

    let part_size = get_part_size(file_size, 0);
    let part_count = (file_size + part_size - 1) / part_size;

    let mut file = std::fs::File::open(local_path)?;

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

        let part_number = i + 1;
        step!("上传分片 {}/{}", part_number, part_count);

        let client = reqwest::Client::new();

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "Content-Type",
            format!("text/plain;name={}", file_name).parse().unwrap(),
        );
        headers.insert("contentSize", file_size.to_string().parse().unwrap());
        headers.insert(
            "range",
            format!("bytes={}-{}", i * part_size, i * part_size + read_size - 1)
                .parse()
                .unwrap(),
        );
        headers.insert("uploadtaskID", upload_task_id.parse().unwrap());
        headers.insert("rangeType", "0".parse().unwrap());

        let resp = client
            .post(upload_url)
            .headers(headers)
            .body(buffer)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(ClientError::Api(format!(
                "分片 {} 上传失败: {}",
                part_number,
                resp.status()
            )));
        }
    }

    Ok(())
}
