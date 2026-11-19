use super::{HttpClientWrapper, headers::build_upload_part_headers};
use crate::client::ClientError;
use futures_util::Stream;
use reqwest::{Body, Response};
use std::io;

pub(crate) struct DownloadLink<'a> {
    pub(super) client: &'a HttpClientWrapper,
    pub(super) url: String,
    pub name: Option<String>,
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

impl HttpClientWrapper {
    pub(crate) async fn put_upload_part(
        &self,
        url: &str,
        part_number: i32,
        size: u64,
        stream: impl Stream<Item = Result<Vec<u8>, io::Error>> + Send + 'static,
    ) -> Result<(), ClientError> {
        let response = self
            .client
            .put(url)
            .headers(build_upload_part_headers(size))
            .body(Body::wrap_stream(stream))
            .send()
            .await
            .map_err(reqwest::Error::without_url)?;

        let status = response.status().as_u16();
        if status != 200 {
            return Err(ClientError::Api(format!(
                "分片 {part_number} 上传失败: HTTP {status}"
            )));
        }
        Ok(())
    }
}
