use super::personal::client::{ListingRetry, PersonalClient};
use crate::client::api::response::{
    format_api_error, is_mutation_successful, is_response_successful, json_api_error,
};
use crate::client::http::HttpClientWrapper;
use crate::utils::remote_file::is_non_root_file_id;
use crate::utils::upload_parts::UploadLayout;
use crate::{
    config::{Config, PersonalConfig},
    debug,
    error::ClientError,
    protocol,
    {
        models::common::BaseResp,
        models::upload::{PersonalPartInfo, PersonalUploadResp},
    },
};
use std::{collections::HashMap, ops::Range, path::Path};

fn upload_creation_error(response: &BaseResp, file_size: i64) -> ClientError {
    let code = response.code.as_deref();
    let message = format_api_error(
        "创建上传任务失败",
        response.message.as_deref().unwrap_or("未知错误"),
        code,
    );
    let message = if code == Some(protocol::UPLOAD_ENTITLEMENT_ERROR_CODE) {
        format!("{message}；文件大小 {file_size} 字节，请核对账号当前的单文件上传额度")
    } else {
        message
    };
    ClientError::Api(message)
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

pub(crate) struct UploadFile<'a> {
    pub(crate) local_path: &'a Path,
    pub(crate) file_name: &'a str,
    pub(crate) layout: UploadLayout,
    pub(crate) parent_file_id: &'a str,
    pub(crate) content_hash: &'a str,
}

pub(crate) struct CreatedUpload {
    pub(crate) file_id: String,
    pub(crate) name: Option<String>,
    pub(crate) pending: Option<PendingUpload>,
}

pub(crate) struct PendingUpload {
    pub(crate) upload_id: String,
    pub(crate) parts: Vec<PersonalPartInfo>,
}

impl PersonalClient<'_> {
    pub(crate) fn upload_part_size(&self, file_size: i64) -> i64 {
        get_part_size(file_size, self.config.custom_upload_part_size)
    }

    pub(crate) async fn create_upload(
        &self,
        file: UploadFile<'_>,
    ) -> Result<CreatedUpload, ClientError> {
        let file_size = file.layout.file_size();
        let body = build_upload_body(file);
        let response: PersonalUploadResp = self.request(protocol::CREATE_FILE, &body).await?;
        if !response.base.success {
            return Err(upload_creation_error(&response.base, file_size));
        }
        let data = response
            .data
            .ok_or_else(|| ClientError::Api("服务端未返回上传结果".into()))?;
        let file_id = data
            .file_id
            .filter(|id| is_non_root_file_id(id))
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
        layout: UploadLayout,
    ) -> Result<HashMap<i32, String>, ClientError> {
        let mut upload_urls = HashMap::new();

        let body = serde_json::json!({
            "fileId": file_id,
            "uploadId": upload_id,
            "partInfos": build_part_infos(parts, layout),
            "commonAccountInfo": {
                "account": self.config.account,
                "accountType": protocol::PERSONAL_ACCOUNT_TYPE
            }
        });

        let resp_json: serde_json::Value = self.request(protocol::UPLOAD_URL, &body).await?;

        if !is_response_successful(&resp_json) {
            return Err(json_api_error(
                "获取分片上传地址失败",
                &resp_json,
                "未知错误",
            ));
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
            "contentHashAlgorithm": protocol::CONTENT_HASH_SHA256,
            "uploadId": upload_id,
            "fileId": file_id,
        });

        let resp_json: serde_json::Value = self.request(protocol::COMPLETE_UPLOAD, &body).await?;
        if !is_mutation_successful(&resp_json) {
            return Err(json_api_error(
                "完成上传失败",
                &resp_json,
                "服务端未确认上传完成",
            ));
        }
        Ok(())
    }
}

fn build_upload_body(file: UploadFile<'_>) -> serde_json::Value {
    let UploadFile {
        local_path,
        file_name,
        layout,
        parent_file_id,
        content_hash,
    } = file;
    let content_type = get_content_type(local_path);

    serde_json::json!({
        "contentHash": content_hash,
        "contentHashAlgorithm": protocol::CONTENT_HASH_SHA256,
        "contentType": content_type,
        "parallelUpload": false,
        "partInfos": build_part_infos(0..layout.part_count().min(protocol::UPLOAD_PARTS_PER_REQUEST), layout),
        "size": layout.file_size(),
        "parentFileId": parent_file_id,
        "name": file_name,
        "type": protocol::FILE_TYPE_FILE,
        "fileRenameMode": protocol::UPLOAD_AUTO_RENAME
    })
}

fn build_part_infos(parts: Range<i64>, layout: UploadLayout) -> Vec<serde_json::Value> {
    parts
        .map(|index| {
            let (offset, size) = layout.part_offset_and_size(index);
            serde_json::json!({
                "partNumber": (index + 1) as i32,
                "partSize": size,
                "parallelHashCtx": { "partOffset": offset },
            })
        })
        .collect()
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
    let config = PersonalConfig::new(config)?;
    let layout = UploadLayout::new(file_size, part_size)?;
    if !(0..=layout.part_count()).contains(&part_count) {
        return Err(ClientError::Api("请求的分片数量超出文件分片范围".into()));
    }
    let client = HttpClientWrapper::new();
    let personal = PersonalClient::with_host(config, host, &client, ListingRetry::None);
    let mut upload_urls = HashMap::new();

    for batch_start in (0..part_count).step_by(protocol::UPLOAD_PARTS_PER_REQUEST as usize) {
        let batch_end = (batch_start + protocol::UPLOAD_PARTS_PER_REQUEST).min(part_count);
        upload_urls.extend(
            personal
                .upload_urls(file_id, upload_id, batch_start..batch_end, layout)
                .await?,
        );
    }
    Ok(upload_urls)
}

#[cfg(test)]
mod tests {
    use super::UploadFile;
    use crate::client::api::headers::PC_CLIENT_INFO;
    use crate::client::api::personal::client::{ListingRetry, PersonalClient};
    use crate::client::http::HttpClientWrapper;
    use crate::config::PersonalConfig;
    use crate::utils::upload_parts::UploadLayout;
    use httpmock::prelude::*;
    use serde_json::json;

    #[tokio::test]
    async fn large_upload_initialization_uses_pc_identity_and_preserves_entitlement_errors() {
        let server = MockServer::start();
        let config = crate::config::Config {
            personal_cloud_host: Some(server.base_url()),
            ..Default::default()
        };
        let client = HttpClientWrapper::new();
        let personal = PersonalClient::with_host(
            PersonalConfig::new(&config).unwrap(),
            server.base_url(),
            &client,
            ListingRetry::None,
        );
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
                    layout: UploadLayout::new(size, personal.upload_part_size(size)).unwrap(),
                    parent_file_id: "/",
                    content_hash: "fictional-hash",
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
                layout: UploadLayout::new(1, personal.upload_part_size(1)).unwrap(),
                parent_file_id: "/",
                content_hash: "fictional-hash",
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
            let personal = PersonalClient::with_host(
                PersonalConfig::new(&config).unwrap(),
                server.base_url(),
                &client,
                ListingRetry::None,
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
                        layout: UploadLayout::new(size, personal.upload_part_size(size)).unwrap(),
                        parent_file_id: "/",
                        content_hash: "fictional-hash",
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
}
