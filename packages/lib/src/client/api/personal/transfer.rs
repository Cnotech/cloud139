use super::client::PersonalClient;
use crate::client::api::response::format_api_error;
use crate::client::http::{HttpClientWrapper, insert_static_headers};
use crate::error::ClientError;
use crate::protocol;
use futures_util::{Stream, StreamExt};
use reqwest::{
    Body, Response, StatusCode,
    header::{CONTENT_LENGTH, HeaderMap, HeaderValue},
};
use std::io;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;
use tokio::time::{error::Elapsed, timeout};

const UPLOAD_IDLE_TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) struct DownloadLink<'a> {
    client: &'a HttpClientWrapper,
    pub(in crate::client::api) url: String,
    pub(crate) name: Option<String>,
}

impl DownloadLink<'_> {
    pub(crate) async fn open(&self) -> Result<DownloadContent, ClientError> {
        // 下载链接带签名，错误中不能包含 URL。
        let response = self
            .client
            .client
            .get(&self.url)
            .send()
            .await
            .and_then(Response::error_for_status)
            .map_err(reqwest::Error::without_url)?;
        Ok(DownloadContent { response })
    }

    #[cfg(test)]
    pub(crate) fn for_test<'a>(client: &'a HttpClientWrapper, url: &str) -> DownloadLink<'a> {
        DownloadLink {
            client,
            url: url.to_owned(),
            name: None,
        }
    }
}

pub(crate) struct DownloadContent {
    response: Response,
}

impl DownloadContent {
    pub(crate) fn content_length(&self) -> Option<u64> {
        self.response.content_length()
    }

    pub(crate) async fn next_chunk(
        &mut self,
    ) -> Result<Option<impl AsRef<[u8]> + use<>>, ClientError> {
        Ok(self
            .response
            .chunk()
            .await
            .map_err(reqwest::Error::without_url)?)
    }
}

impl PersonalClient<'_> {
    pub(crate) async fn download_link(
        &self,
        file_id: &str,
    ) -> Result<DownloadLink<'_>, ClientError> {
        #[derive(serde::Deserialize)]
        struct Response {
            #[serde(flatten)]
            base: crate::models::common::BaseResp,
            data: Option<crate::models::download::DownloadUrlData>,
        }

        let response: Response = self
            .request(
                protocol::DOWNLOAD_URL,
                &serde_json::json!({ "fileId": file_id }),
            )
            .await?;
        if !response.base.success {
            return Err(ClientError::Api(format_api_error(
                "获取下载链接失败",
                response.base.message.as_deref().unwrap_or("未知错误"),
                None,
            )));
        }
        let data = response
            .data
            .ok_or_else(|| ClientError::Api("获取下载链接失败: 无数据".into()))?;
        let url = data
            .cdn_url
            .filter(|url| !url.is_empty())
            .or_else(|| data.url.filter(|url| !url.is_empty()))
            .ok_or_else(|| ClientError::Api("获取下载链接失败: URL为空".into()))?;
        Ok(DownloadLink {
            client: self.http_client,
            url,
            name: data.file_name,
        })
    }
}

fn build_upload_part_headers(size: u64) -> HeaderMap {
    let mut headers = HeaderMap::new();
    insert_static_headers(&mut headers, protocol::UPLOAD_PART_HEADERS);
    headers.insert(CONTENT_LENGTH, HeaderValue::from(size));
    headers
}

pub(crate) async fn put_upload_part(
    client: &HttpClientWrapper,
    url: &str,
    part_number: i32,
    size: u64,
    stream: impl Stream<Item = Result<Vec<u8>, io::Error>> + Send + 'static,
) -> Result<(), ClientError> {
    let activity = Arc::new(Notify::new());
    let stream_activity = Arc::clone(&activity);
    let stream = stream.inspect(move |chunk| {
        if chunk.as_ref().is_ok_and(|bytes| !bytes.is_empty()) {
            stream_activity.notify_one();
        }
    });
    let request = client
        .client
        .put(url)
        .headers(build_upload_part_headers(size))
        .body(Body::wrap_stream(stream))
        .send();
    let response = with_upload_idle_timeout(request, &activity)
        .await
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::TimedOut,
                format!(
                    "分片 {part_number} 上传超时，连续 {} 秒未发送数据或收到响应",
                    UPLOAD_IDLE_TIMEOUT.as_secs()
                ),
            )
        })?
        .map_err(|error| {
            crate::debug!(
                "发送上传分片请求失败 part={} connect={} timeout={}",
                part_number,
                error.is_connect(),
                error.is_timeout()
            );
            error.without_url()
        })?;

    let status = response.status();
    if status != StatusCode::OK {
        return Err(ClientError::Api(format!(
            "分片 {part_number} 上传失败: HTTP {}",
            status.as_u16()
        )));
    }
    Ok(())
}

async fn with_upload_idle_timeout<T>(
    request: impl Future<Output = T>,
    activity: &Notify,
) -> Result<T, Elapsed> {
    tokio::select! {
        biased;
        response = request => Ok(response),
        expired = async {
            loop {
                timeout(UPLOAD_IDLE_TIMEOUT, activity.notified()).await?;
            }
        } => expired,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::pending;
    use tokio::time::{Instant, sleep};

    #[tokio::test(start_paused = true)]
    async fn upload_can_take_longer_than_the_idle_timeout_with_progress() {
        let activity = Notify::new();
        let request = async {
            sleep(Duration::from_secs(20)).await;
            activity.notify_one();
            sleep(Duration::from_secs(20)).await;
        };

        with_upload_idle_timeout(request, &activity).await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn upload_times_out_without_progress_after_sending_data() {
        let activity = Notify::new();
        let start = Instant::now();
        let request = async {
            sleep(Duration::from_secs(20)).await;
            activity.notify_one();
            pending::<()>().await;
        };

        assert!(with_upload_idle_timeout(request, &activity).await.is_err());
        assert_eq!(start.elapsed(), Duration::from_secs(50));
    }
}
