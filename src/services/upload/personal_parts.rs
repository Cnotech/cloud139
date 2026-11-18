use crate::client::ClientError;
use crate::domain::StorageType;

pub(super) const PART_BATCH_SIZE: i64 = 100;

pub(super) fn part_offset_and_size(index: i64, part_size: i64, file_size: i64) -> (i64, i64) {
    let offset = index * part_size;
    (offset, part_size.min(file_size - offset))
}

pub(super) fn part_infos(
    parts: std::ops::Range<i64>,
    part_size: i64,
    file_size: i64,
) -> Vec<serde_json::Value> {
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

    for batch_start in (0..part_count).step_by(PART_BATCH_SIZE as usize) {
        let batch_end = (batch_start + PART_BATCH_SIZE).min(part_count);
        upload_urls.extend(
            get_upload_url_batch(
                config,
                host,
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

pub(super) async fn get_upload_url_batch(
    config: &crate::config::Config,
    host: &str,
    file_id: &str,
    upload_id: &str,
    parts: std::ops::Range<i64>,
    part_size: i64,
    file_size: i64,
) -> Result<std::collections::HashMap<i32, String>, ClientError> {
    let mut upload_urls = std::collections::HashMap::new();

    let url = format!("{}/file/getUploadUrl", host);

    let body = serde_json::json!({
        "fileId": file_id,
        "uploadId": upload_id,
        "partInfos": part_infos(parts, part_size, file_size),
        "commonAccountInfo": {
            "account": config.account,
            "accountType": 1
        }
    });

    let resp_json: serde_json::Value =
        crate::client::api::personal_api_request(config, &url, body, StorageType::PersonalNew)
            .await?;

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

pub async fn upload_single_part(
    upload_urls: &std::collections::HashMap<i32, String>,
    part_number: i32,
    buffer: &[u8],
    pb: Option<indicatif::ProgressBar>,
) -> Result<(), ClientError> {
    let client = crate::client::api::HttpClientWrapper::new();
    upload_part(
        &client.client,
        upload_urls,
        part_number,
        std::io::Cursor::new(buffer.to_vec()),
        buffer.len() as u64,
        pb,
    )
    .await
}

pub(super) async fn upload_part(
    client: &reqwest::Client,
    upload_urls: &std::collections::HashMap<i32, String>,
    part_number: i32,
    reader: impl tokio::io::AsyncRead + Send + Unpin + 'static,
    size: u64,
    pb: Option<indicatif::ProgressBar>,
) -> Result<(), ClientError> {
    use tokio::io::AsyncReadExt;

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
    let resp = client
        .put(upload_url)
        .header("Content-Type", "application/octet-stream")
        .header("Content-Length", size)
        .header("Origin", "https://yun.139.com")
        .header("Referer", "https://yun.139.com/")
        .body(reqwest::Body::wrap_stream(stream))
        .send()
        .await
        .map_err(reqwest::Error::without_url)?;

    let status = resp.status().as_u16();
    if status != 200 {
        return Err(ClientError::Api(format!(
            "分片 {} 上传失败: HTTP {}",
            part_number, status
        )));
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
    use crate::debug;

    debug!("所有分片上传完成");

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

    if !crate::services::directory_service::mutation_succeeded(&resp_json) {
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
