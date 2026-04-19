use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct BaseResp {
    pub success: bool,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ApiResult {
    #[serde(rename = "resultCode")]
    pub result_code: String,
    #[serde(rename = "resultDesc")]
    pub result_desc: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CommonAccountInfo {
    pub account: String,
    #[serde(rename = "accountType")]
    pub account_type: i32,
}

#[derive(Debug, Deserialize)]
pub struct CreateBatchOprTaskResp {
    pub result: ApiResult,
    #[serde(rename = "taskID")]
    pub task_id: String,
}

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
