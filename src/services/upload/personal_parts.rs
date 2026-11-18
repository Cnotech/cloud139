use crate::client::{ClientError, api::HttpClientWrapper};

pub use crate::client::api::upload::get_upload_urls;

pub(super) async fn upload_part(
    client: &HttpClientWrapper,
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
    client
        .put_upload_part(upload_url, part_number, size, stream)
        .await
}
