use super::client::{ListingRetry, PersonalClient};
use crate::client::{
    ClientError,
    http::{HttpClientWrapper, insert_static_headers},
};
use crate::config::Config;
use crate::error::format_api_error;
use crate::protocol;
use futures_util::Stream;
use reqwest::{
    Body, Response, StatusCode,
    header::{CONTENT_LENGTH, HeaderMap, HeaderValue},
};
use std::io;

pub(crate) struct DownloadLink<'a> {
    client: &'a HttpClientWrapper,
    url: String,
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
            base: crate::models::BaseResp,
            data: Option<crate::models::DownloadUrlData>,
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

pub async fn get_personal_download_link(
    config: &Config,
    file_id: &str,
) -> Result<String, ClientError> {
    let client = HttpClientWrapper::new();
    let personal = PersonalClient::connect(config, &client, ListingRetry::None).await?;
    personal.download_link(file_id).await.map(|link| link.url)
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
    let response = client
        .client
        .put(url)
        .headers(build_upload_part_headers(size))
        .body(Body::wrap_stream(stream))
        .send()
        .await
        .map_err(reqwest::Error::without_url)?;

    let status = response.status();
    if status != StatusCode::OK {
        return Err(ClientError::Api(format!(
            "分片 {part_number} 上传失败: HTTP {}",
            status.as_u16()
        )));
    }
    Ok(())
}
