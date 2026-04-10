use serde::{Deserialize, Serialize};

use super::common::{BaseResp, CommonAccountInfo};

#[derive(Debug, Deserialize)]
pub struct PartInfo {
    #[serde(rename = "partNumber")]
    pub part_number: i64,
    #[serde(rename = "partSize")]
    pub part_size: i64,
    #[serde(rename = "parallelHashCtx")]
    pub parallel_hash_ctx: ParallelHashCtx,
}

#[derive(Debug, Deserialize)]
pub struct ParallelHashCtx {
    #[serde(rename = "partOffset")]
    pub part_offset: i64,
}

#[derive(Debug, Deserialize)]
pub struct PersonalUploadResp {
    #[serde(flatten)]
    pub base: BaseResp,
    #[serde(default)]
    pub data: Option<PersonalUploadData>,
}

#[derive(Debug, Deserialize)]
pub struct PersonalUploadData {
    #[serde(rename = "fileId", default)]
    pub file_id: Option<String>,
    #[serde(rename = "fileName", default)]
    pub file_name: Option<String>,
    #[serde(rename = "partInfos", default)]
    pub part_infos: Option<Vec<PersonalPartInfo>>,
    #[serde(default)]
    pub exist: Option<bool>,
    #[serde(rename = "rapidUpload", default)]
    pub rapid_upload: Option<bool>,
    #[serde(rename = "uploadId", default)]
    pub upload_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonalPartInfo {
    pub part_number: i32,
    #[serde(rename = "uploadUrl")]
    pub upload_url: String,
}

#[derive(Debug, Serialize)]
pub struct UploadRequest {
    #[serde(rename = "contentHash")]
    pub content_hash: String,
    #[serde(rename = "contentHashAlgorithm")]
    pub content_hash_algorithm: String,
    #[serde(rename = "size")]
    pub size: i64,
    #[serde(rename = "parentFileId")]
    pub parent_file_id: String,
    #[serde(rename = "name")]
    pub name: String,
    #[serde(rename = "fileRenameMode")]
    pub file_rename_mode: Option<String>,
    #[serde(rename = "type")]
    pub file_type: Option<String>,
    #[serde(rename = "contentType")]
    pub content_type: Option<String>,
    #[serde(rename = "commonAccountInfo")]
    pub common_account_info: Option<CommonAccountInfo>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FamilyCreateFolderRequest {
    #[serde(rename = "catalogName")]
    pub catalog_name: String,
    #[serde(rename = "parentCatalogID")]
    pub parent_catalog_id: String,
}
