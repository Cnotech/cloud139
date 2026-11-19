use serde::Deserialize;

use super::common::BaseResp;

#[derive(Debug, Deserialize)]
pub struct DownloadUrlResp {
    #[serde(flatten)]
    pub base: BaseResp,
    pub data: DownloadUrlData,
}

#[derive(Debug, Deserialize)]
pub struct DownloadUrlData {
    #[serde(default)]
    pub url: Option<String>,
    #[serde(rename = "cdnUrl", default)]
    pub cdn_url: Option<String>,
    #[serde(rename = "fileName", default)]
    pub file_name: Option<String>,
}
