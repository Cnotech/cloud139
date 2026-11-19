use serde::Deserialize;

use super::common::BaseResp;

#[derive(Debug, Deserialize)]
pub struct CreateFolderResp {
    #[serde(flatten)]
    pub base: BaseResp,
    pub data: CreateFolderData,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateFolderData {
    #[serde(rename = "fileId")]
    pub file_id: String,
    #[serde(rename = "fileName")]
    pub file_name: String,
}

#[derive(Debug, Deserialize)]
pub struct QueryFileResp {
    #[serde(flatten)]
    pub base: BaseResp,
    pub data: QueryFileData,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryFileData {
    #[serde(rename = "fileId")]
    pub file_id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub file_type: String,
    #[serde(rename = "contentHash", default)]
    pub content_hash: Option<String>,
    #[serde(rename = "contentHashAlgorithm", default)]
    pub content_hash_algorithm: Option<String>,
}
