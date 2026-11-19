use super::checked_upload_size;
use crate::client::api::personal::client::PersonalClient;
use crate::client::api::personal::transfer::put_upload_part;
use crate::client::api::upload::UploadFile;
use crate::client::http::HttpClientWrapper;
use crate::error::ClientError;
use crate::models::upload::{PersonalPartInfo, UploadLayout};
use crate::progress::Progress;
use crate::protocol;
use std::collections::HashMap;
use std::path::Path;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncSeekExt, SeekFrom};

pub(in crate::services) struct UploadReceipt {
    pub(in crate::services) name: Option<String>,
    pub(in crate::services) id: String,
    pub(in crate::services) size: u64,
    pub(in crate::services) sha256: String,
}

pub(in crate::services) async fn transfer_to_parent(
    personal: &PersonalClient<'_>,
    local_path: &Path,
    parent_file_id: &str,
    file_name: &str,
    file_size: i64,
    pb: Option<Progress>,
) -> Result<UploadReceipt, ClientError> {
    if file_size < 0 {
        return Err(ClientError::InvalidFilePath);
    }
    let file_size = checked_upload_size(local_path)?;
    let layout = UploadLayout::new(file_size, personal.upload_part_size(file_size))?;
    crate::debug!("开始计算文件 SHA-256");
    let path = local_path.to_owned();
    let content_hash =
        tokio::task::spawn_blocking(move || crate::utils::crypto::calc_file_sha256(path))
            .await
            .map_err(|error| ClientError::Other(format!("计算文件哈希失败: {error}")))??;
    let response = personal
        .create_upload(UploadFile {
            local_path,
            file_name,
            layout,
            parent_file_id,
            content_hash: &content_hash,
        })
        .await?;
    let receipt = UploadReceipt {
        name: response.name,
        id: response.file_id,
        size: file_size as u64,
        sha256: content_hash,
    };
    if let Some(pending) = response.pending {
        crate::debug!("开始上传分片");
        upload_parts(UploadPartsParams {
            personal,
            local_path,
            upload_id: &pending.upload_id,
            file_id: &receipt.id,
            layout,
            content_hash: &receipt.sha256,
            initial_parts: &pending.parts,
            pb,
        })
        .await?;
    } else {
        crate::debug!("服务端确认秒传或创建空文件");
    }
    Ok(receipt)
}

struct UploadPartsParams<'a> {
    personal: &'a PersonalClient<'a>,
    local_path: &'a Path,
    upload_id: &'a str,
    file_id: &'a str,
    layout: UploadLayout,
    content_hash: &'a str,
    initial_parts: &'a [PersonalPartInfo],
    pb: Option<Progress>,
}

async fn upload_parts(params: UploadPartsParams<'_>) -> Result<(), ClientError> {
    let personal = params.personal;
    let local_path = params.local_path;
    let upload_id = params.upload_id;
    let file_id = params.file_id;
    let layout = params.layout;
    let file_size = layout.file_size();
    let content_hash = params.content_hash;
    let file = tokio::fs::File::open(local_path).await?;
    let data_client = HttpClientWrapper::new();
    if file.metadata().await?.len() < file_size as u64 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "本地文件在上传前被截断",
        )
        .into());
    }
    let part_count = layout.part_count();

    for batch_start in (0..part_count).step_by(protocol::UPLOAD_PARTS_PER_REQUEST as usize) {
        let batch_end = (batch_start + protocol::UPLOAD_PARTS_PER_REQUEST).min(part_count);
        let upload_urls = if batch_start == 0 {
            params
                .initial_parts
                .iter()
                .map(|part| (part.part_number, part.upload_url.clone()))
                .collect()
        } else {
            personal
                .upload_urls(file_id, upload_id, batch_start..batch_end, layout)
                .await?
        };
        for i in batch_start..batch_end {
            let (offset, read_size) = layout.part_offset_and_size(i);
            let mut part = file.try_clone().await?;
            part.seek(SeekFrom::Start(offset as u64)).await?;
            let part_number = (i + 1) as i32;
            crate::debug!("正在上传分片 part={}/{}", part_number, part_count);
            upload_part(
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

async fn upload_part(
    client: &HttpClientWrapper,
    upload_urls: &HashMap<i32, String>,
    part_number: i32,
    reader: impl AsyncRead + Send + Unpin + 'static,
    size: u64,
    pb: Option<Progress>,
) -> Result<(), ClientError> {
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
    put_upload_part(client, upload_url, part_number, size, stream).await
}

#[cfg(test)]
mod tests {
    use super::{UploadPartsParams, upload_parts};
    use crate::client::api::personal::client::{ListingRetry, PersonalClient};
    use crate::client::http::HttpClientWrapper;
    use crate::config::personal::PersonalConfig;
    use crate::error::ClientError;
    use crate::models::upload::UploadLayout;
    use httpmock::prelude::*;
    use serde_json::json;

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
            .map(|part_number| crate::models::upload::PersonalPartInfo {
                part_number,
                upload_url: server.url("/initial"),
            })
            .collect();
        let client = HttpClientWrapper::new();
        let personal = PersonalClient::with_host(
            PersonalConfig::new(&config).unwrap(),
            server.base_url(),
            &client,
            ListingRetry::None,
        );
        upload_parts(UploadPartsParams {
            personal: &personal,
            local_path: source.path(),
            upload_id: "upload",
            file_id: "new",
            layout: UploadLayout::new(101, 1).unwrap(),
            content_hash: "fictional-hash",
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
        let client = HttpClientWrapper::new();
        let personal = PersonalClient::with_host(
            PersonalConfig::new(&config).unwrap(),
            server.base_url(),
            &client,
            ListingRetry::None,
        );
        let error = upload_parts(UploadPartsParams {
            personal: &personal,
            local_path: source.path(),
            upload_id: "upload",
            file_id: "new",
            layout: UploadLayout::new(2, 2).unwrap(),
            content_hash: "fictional-hash",
            initial_parts: &[crate::models::upload::PersonalPartInfo {
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
}
