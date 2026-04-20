use crate::client::ClientError;
use crate::client::endpoints::family;
use crate::commands::upload::UploadPartParams;
use crate::info;
use indicatif::ProgressBar;
use log::debug;
use std::io::{Read, Seek};

pub async fn upload(
    config: &crate::config::Config,
    local_path: &std::path::Path,
    remote_path: &str,
    file_name: &str,
    file_size: i64,
    pb: Option<ProgressBar>,
) -> Result<(), ClientError> {
    let client = crate::client::Client::new(config.clone());
    let url = family::orchestration::GET_FILE_UPLOAD_URL;

    let upload_path = resolve_upload_path(config, remote_path);
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

    if !is_success(&resp) {
        return Err(ClientError::Api(format!("获取上传URL失败: {:?}", resp)));
    }

    let upload_url = extract_upload_url(&resp)?;
    let upload_task_id = extract_upload_task_id(&resp)?;

    info!("开始上传文件到家庭云...");
    upload_file(
        local_path,
        upload_url,
        upload_task_id,
        file_size,
        file_name,
        pb,
    )
    .await?;

    debug!("上传完成!");
    Ok(())
}

fn resolve_upload_path(config: &crate::config::Config, remote_path: &str) -> String {
    if remote_path == "/" || remote_path.is_empty() {
        if let Some(ref root_path) = config.root_folder_id {
            root_path.clone()
        } else {
            "0".to_string()
        }
    } else {
        remote_path.to_string()
    }
}

fn is_success(resp: &serde_json::Value) -> bool {
    resp.get("result")
        .and_then(|r| r.get("resultCode"))
        .and_then(|c| c.as_str())
        == Some("0")
}

fn extract_upload_url(resp: &serde_json::Value) -> Result<&str, ClientError> {
    resp.pointer("/data/uploadResult/redirectionUrl")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ClientError::Api("未找到上传URL".to_string()))
}

fn extract_upload_task_id(resp: &serde_json::Value) -> Result<&str, ClientError> {
    resp.pointer("/data/uploadResult/uploadTaskID")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ClientError::Api("未找到上传任务ID".to_string()))
}

async fn upload_file(
    local_path: &std::path::Path,
    upload_url: &str,
    upload_task_id: &str,
    file_size: i64,
    file_name: &str,
    pb: Option<ProgressBar>,
) -> Result<(), ClientError> {
    let part_size = crate::commands::upload::get_part_size(file_size, 0);
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
        debug!("上传分片 {}/{}", part_number, part_count);

        upload_part(&UploadPartParams {
            upload_url,
            upload_task_id,
            buffer: &buffer[..bytes_read],
            part_number,
            part_offset: part_size,
            read_size,
            file_name,
            total_size: file_size,
        })
        .await?;

        if let Some(pb) = &pb {
            pb.inc(bytes_read as u64);
        }
    }

    if let Some(pb) = pb {
        pb.finish_and_clear();
    }

    Ok(())
}

async fn upload_part(params: &UploadPartParams<'_>) -> Result<(), ClientError> {
    let client = reqwest::Client::new();

    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        "Content-Type",
        reqwest::header::HeaderValue::from_str(&format!("text/plain;name={}", params.file_name))
            .map_err(|e| ClientError::InvalidHeader(e.to_string()))?,
    );
    headers.insert(
        "contentSize",
        reqwest::header::HeaderValue::from_str(&params.total_size.to_string())
            .map_err(|e| ClientError::InvalidHeader(e.to_string()))?,
    );
    headers.insert(
        "range",
        reqwest::header::HeaderValue::from_str(&format!(
            "bytes={}-{}",
            params.part_offset,
            params.part_offset + params.read_size - 1
        ))
        .map_err(|e| ClientError::InvalidHeader(e.to_string()))?,
    );
    headers.insert(
        "uploadtaskID",
        reqwest::header::HeaderValue::from_str(params.upload_task_id)
            .map_err(|e| ClientError::InvalidHeader(e.to_string()))?,
    );
    headers.insert("rangeType", reqwest::header::HeaderValue::from_static("0"));

    let resp = client
        .post(params.upload_url)
        .headers(headers)
        .body(params.buffer.to_vec())
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(ClientError::Api(format!(
            "分片 {} 上传失败: {}",
            params.part_number,
            resp.status()
        )));
    }

    Ok(())
}
