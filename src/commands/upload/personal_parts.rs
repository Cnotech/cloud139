use crate::client::{ClientError, StorageType};

pub async fn get_upload_urls(
    config: &crate::config::Config,
    host: &str,
    file_id: &str,
    upload_id: &str,
    part_count: i64,
    part_size: i64,
    file_size: i64,
) -> Result<std::collections::HashMap<i32, String>, ClientError> {
    use std::collections::HashMap;

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
            crate::client::api::personal_api_request(config, &url, body, StorageType::PersonalNew).await?;

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

    Ok(upload_urls)
}

pub async fn upload_single_part(
    upload_urls: &std::collections::HashMap<i32, String>,
    part_number: i32,
    buffer: &[u8],
) -> Result<(), ClientError> {
    use crate::error;

    let upload_url = upload_urls
        .get(&part_number)
        .cloned()
        .ok_or_else(|| ClientError::Api(format!("找不到分片 {} 的上传URL", part_number)))?;

    let buffer = buffer.to_vec();
    let resp_code = tokio::task::spawn_blocking(move || {
        ureq::put(&upload_url)
            .header("Content-Type", "application/octet-stream")
            .send(&buffer)
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

    Ok(())
}

pub async fn confirm_upload(
    config: &crate::config::Config,
    host: &str,
    file_id: &str,
    upload_id: &str,
    content_hash: &str,
) -> Result<(), ClientError> {
    use crate::info;
    use crate::step;

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