use super::personal::{CREATE_FILE, mutation_succeeded};
use super::{HttpClientWrapper, ListingRetry, PersonalClient};
use crate::{
    client::ClientError,
    config::Config,
    debug,
    models::{PersonalPartInfo, PersonalUploadResp},
};
use std::{collections::HashMap, ops::Range, path::Path};

pub(crate) struct UploadFile<'a> {
    pub local_path: &'a Path,
    pub file_name: &'a str,
    pub file_size: i64,
    pub parent_file_id: &'a str,
    pub content_hash: &'a str,
    pub part_size: i64,
}

pub(crate) struct CreatedUpload {
    pub file_id: String,
    pub name: Option<String>,
    pub pending: Option<PendingUpload>,
}

pub(crate) struct PendingUpload {
    pub upload_id: String,
    pub parts: Vec<PersonalPartInfo>,
}

impl PersonalClient<'_> {
    pub(crate) fn upload_part_size(&self, file_size: i64) -> i64 {
        get_part_size(file_size, self.config.custom_upload_part_size)
    }

    pub(crate) async fn create_upload(
        &self,
        file: UploadFile<'_>,
    ) -> Result<CreatedUpload, ClientError> {
        let file_size = file.file_size;
        let body = build_upload_body(file)?;
        let response: PersonalUploadResp = self.request(CREATE_FILE, &body).await?;
        if !response.base.success {
            let quota_hint = if response.base.code.as_deref() == Some("04010319") {
                format!("；文件大小 {file_size} 字节，请核对账号当前的单文件上传额度")
            } else {
                String::new()
            };
            return Err(ClientError::Api(format!(
                "创建上传任务失败: {}{}{}",
                response.base.message.as_deref().unwrap_or("未知错误"),
                response
                    .base
                    .code
                    .as_deref()
                    .map(|code| format!("（错误码: {code}）"))
                    .unwrap_or_default(),
                quota_hint,
            )));
        }
        let data = response
            .data
            .ok_or_else(|| ClientError::Api("服务端未返回上传结果".into()))?;
        let file_id = data
            .file_id
            .filter(|id| !id.is_empty() && id != "/")
            .ok_or_else(|| ClientError::Api("服务端未返回新文件 ID".into()))?;
        let pending = match data.part_infos.filter(|parts| !parts.is_empty()) {
            Some(parts) => {
                let upload_id = data
                    .upload_id
                    .filter(|id| !id.is_empty())
                    .ok_or_else(|| ClientError::Api("服务端未返回分片上传 ID".into()))?;
                Some(PendingUpload { upload_id, parts })
            }
            None if data.exist == Some(true) || file_size == 0 => None,
            None => {
                return Err(ClientError::Api(
                    "服务端未确认秒传，也未返回上传分片".into(),
                ));
            }
        };
        Ok(CreatedUpload {
            file_id,
            name: data.file_name.filter(|name| !name.is_empty()),
            pending,
        })
    }

    pub(crate) async fn upload_urls(
        &self,
        file_id: &str,
        upload_id: &str,
        parts: Range<i64>,
        part_size: i64,
        file_size: i64,
    ) -> Result<HashMap<i32, String>, ClientError> {
        let mut upload_urls = HashMap::new();

        let body = serde_json::json!({
            "fileId": file_id,
            "uploadId": upload_id,
            "partInfos": build_part_infos(parts, part_size, file_size),
            "commonAccountInfo": {
                "account": self.config.account,
                "accountType": 1
            }
        });

        let resp_json: serde_json::Value = self.request("/file/getUploadUrl", &body).await?;

        if resp_json["success"].as_bool() != Some(true) {
            return Err(ClientError::Api(format!(
                "获取分片上传地址失败: {}（错误码: {}）",
                resp_json["message"].as_str().unwrap_or("未知错误"),
                resp_json["code"].as_str().unwrap_or("未返回")
            )));
        }

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
        Ok(upload_urls)
    }

    pub(crate) async fn complete_upload(
        &self,
        file_id: &str,
        upload_id: &str,
        content_hash: &str,
    ) -> Result<(), ClientError> {
        debug!("所有分片上传完成");

        let body = serde_json::json!({
            "contentHash": content_hash,
            "contentHashAlgorithm": "SHA256",
            "uploadId": upload_id,
            "fileId": file_id,
        });

        let resp_json: serde_json::Value = self.request("/file/complete", &body).await?;
        if !mutation_succeeded(&resp_json) {
            let message = resp_json["message"]
                .as_str()
                .unwrap_or("服务端未确认上传完成");
            let code = resp_json["code"].as_str().unwrap_or("未返回");
            return Err(ClientError::Api(format!(
                "完成上传失败: {message}（错误码: {code}）"
            )));
        }
        Ok(())
    }
}

fn build_upload_body(file: UploadFile<'_>) -> Result<serde_json::Value, ClientError> {
    let UploadFile {
        local_path,
        file_name,
        file_size,
        parent_file_id,
        content_hash,
        part_size,
    } = file;
    let part_count = checked_part_count(file_size, part_size)?;

    let content_type = get_content_type(local_path);

    Ok(serde_json::json!({
        "contentHash": content_hash,
        "contentHashAlgorithm": "SHA256",
        "contentType": content_type,
        "parallelUpload": false,
        "partInfos": build_part_infos(0..part_count.min(PART_BATCH_SIZE), part_size, file_size),
        "size": file_size,
        "parentFileId": parent_file_id,
        "name": file_name,
        "type": "file",
        "fileRenameMode": "auto_rename"
    }))
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

pub(crate) const PART_BATCH_SIZE: i64 = 100;

pub(crate) fn part_offset_and_size(index: i64, part_size: i64, file_size: i64) -> (i64, i64) {
    let offset = index * part_size;
    (offset, part_size.min(file_size - offset))
}

fn build_part_infos(parts: Range<i64>, part_size: i64, file_size: i64) -> Vec<serde_json::Value> {
    parts
        .map(|index| {
            let (offset, size) = part_offset_and_size(index, part_size, file_size);
            serde_json::json!({
                "partNumber": (index + 1) as i32,
                "partSize": size,
                "parallelHashCtx": { "partOffset": offset },
            })
        })
        .collect()
}

pub(crate) fn checked_part_count(file_size: i64, part_size: i64) -> Result<i64, ClientError> {
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

fn get_content_type(path: &Path) -> &'static str {
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
}

pub async fn get_upload_urls(
    config: &Config,
    host: &str,
    file_id: &str,
    upload_id: &str,
    part_count: i64,
    part_size: i64,
    file_size: i64,
) -> Result<HashMap<i32, String>, ClientError> {
    let client = HttpClientWrapper::new();
    let personal = PersonalClient::with_host(config, host, &client, ListingRetry::None);
    let mut upload_urls = HashMap::new();

    for batch_start in (0..part_count).step_by(PART_BATCH_SIZE as usize) {
        let batch_end = (batch_start + PART_BATCH_SIZE).min(part_count);
        upload_urls.extend(
            personal
                .upload_urls(
                    file_id,
                    upload_id,
                    batch_start..batch_end,
                    part_size,
                    file_size,
                )
                .await?,
        );
    }
    Ok(upload_urls)
}
