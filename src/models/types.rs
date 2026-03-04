use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct BaseResp {
    pub success: bool,
    pub code: String,
    pub message: String,
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
pub struct QueryRoutePolicyResp {
    pub success: bool,
    pub code: String,
    pub message: String,
    pub data: RoutePolicyData,
}

#[derive(Debug, Deserialize)]
pub struct RoutePolicyData {
    #[serde(rename = "routePolicyList")]
    pub route_policy_list: Vec<RoutePolicy>,
}

#[derive(Debug, Deserialize)]
pub struct RoutePolicy {
    #[serde(rename = "siteID")]
    pub site_id: String,
    #[serde(rename = "siteCode")]
    pub site_code: String,
    #[serde(rename = "modName")]
    pub mod_name: String,
    #[serde(rename = "httpUrl")]
    pub http_url: String,
    #[serde(rename = "httpsUrl")]
    pub https_url: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename = "root")]
pub struct RefreshTokenResp {
    #[serde(rename = "return")]
    pub return_code: String,
    pub token: String,
    pub expiretime: i32,
    #[serde(rename = "accessToken")]
    pub access_token: String,
    pub desc: String,
}

#[derive(Debug, Deserialize)]
pub struct PersonalListResp {
    #[serde(flatten)]
    pub base: BaseResp,
    pub data: PersonalListData,
}

#[derive(Debug, Deserialize)]
pub struct PersonalListData {
    pub items: Vec<PersonalFileItem>,
    #[serde(rename = "next_page_cursor")]
    pub next_page_cursor: String,
}

#[derive(Debug, Deserialize)]
pub struct PersonalFileItem {
    #[serde(rename = "fileId")]
    pub file_id: String,
    pub name: String,
    pub size: i64,
    #[serde(rename = "type")]
    pub file_type: String,
    #[serde(rename = "createDate", default)]
    pub create_date: Option<String>,
    #[serde(rename = "updateDate", default)]
    pub update_date: Option<String>,
    #[serde(rename = "lastModified", default)]
    pub last_modified: Option<String>,
    #[serde(rename = "createdTime", default)]
    pub created_time: Option<String>,
    #[serde(rename = "updatedTime", default)]
    pub updated_time: Option<String>,
    #[serde(rename = "thumbnailUrls", default)]
    pub thumbnail_urls: Option<Vec<PersonalThumbnail>>,
}

#[derive(Debug, Deserialize)]
pub struct PersonalThumbnail {
    pub style: String,
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct PersonalUploadResp {
    #[serde(flatten)]
    pub base: BaseResp,
    pub data: PersonalUploadData,
}

#[derive(Debug, Deserialize)]
pub struct PersonalUploadData {
    #[serde(rename = "fileId")]
    pub file_id: String,
    #[serde(rename = "fileName")]
    pub file_name: String,
    #[serde(rename = "partInfos")]
    pub part_infos: Option<Vec<PersonalPartInfo>>,
    pub exist: bool,
    #[serde(rename = "rapidUpload")]
    pub rapid_upload: bool,
    #[serde(rename = "uploadId")]
    pub upload_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonalPartInfo {
    pub part_number: i32,
    #[serde(rename = "uploadUrl")]
    pub upload_url: String,
}

#[derive(Debug, Deserialize)]
pub struct DownloadUrlResp {
    #[serde(flatten)]
    pub base: BaseResp,
    pub data: DownloadUrlData,
}

#[derive(Debug, Deserialize)]
pub struct DownloadUrlData {
    pub url: String,
    #[serde(rename = "cdnUrl")]
    pub cdn_url: Option<String>,
    #[serde(rename = "fileName")]
    pub file_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PersonalDiskInfoResp {
    #[serde(flatten)]
    pub base: BaseResp,
    pub data: PersonalDiskInfoData,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonalDiskInfoData {
    #[serde(rename = "freeDiskSize")]
    pub free_disk_size: String,
    #[serde(rename = "diskSize")]
    pub disk_size: String,
}

#[derive(Debug, Deserialize)]
pub struct QueryContentListResp {
    #[serde(flatten)]
    pub base: BaseResp,
    pub data: QueryContentListData,
}

#[derive(Debug, Deserialize)]
pub struct QueryContentListData {
    pub result: ApiResult,
    pub path: String,
    #[serde(rename = "cloudContentList")]
    pub cloud_content_list: Vec<CloudContent>,
    #[serde(rename = "cloudCatalogList")]
    pub cloud_catalog_list: Vec<CloudCatalog>,
    #[serde(rename = "totalCount")]
    pub total_count: i32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudContent {
    #[serde(rename = "contentID")]
    pub content_id: String,
    #[serde(rename = "contentName")]
    pub content_name: String,
    #[serde(rename = "contentSize")]
    pub content_size: i64,
    #[serde(rename = "createTime")]
    pub create_time: String,
    #[serde(rename = "lastUpdateTime")]
    pub last_update_time: String,
    #[serde(rename = "thumbnailURL")]
    pub thumbnail_url: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudCatalog {
    #[serde(rename = "catalogID")]
    pub catalog_id: String,
    #[serde(rename = "catalogName")]
    pub catalog_name: String,
    #[serde(rename = "createTime")]
    pub create_time: String,
    #[serde(rename = "lastUpdateTime")]
    pub last_update_time: String,
}

#[derive(Debug, Deserialize)]
pub struct FamilyDiskInfoResp {
    #[serde(flatten)]
    pub base: BaseResp,
    pub data: FamilyDiskInfoData,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FamilyDiskInfoData {
    #[serde(rename = "usedSize")]
    pub used_size: String,
    #[serde(rename = "diskSize")]
    pub disk_size: String,
}

#[derive(Debug, Deserialize)]
pub struct QueryGroupContentListResp {
    #[serde(flatten)]
    pub base: BaseResp,
    pub data: QueryGroupContentListData,
}

#[derive(Debug, Deserialize)]
pub struct QueryGroupContentListData {
    pub result: ApiResult,
    #[serde(rename = "getGroupContentResult")]
    pub get_group_content_result: GetGroupContentResult,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetGroupContentResult {
    #[serde(rename = "parentCatalogID")]
    pub parent_catalog_id: String,
    pub catalog_list: Vec<GroupCatalog>,
    pub content_list: Vec<GroupContent>,
    #[serde(rename = "nodeCount")]
    pub node_count: i32,
    pub ctlg_cnt: i32,
    pub cont_cnt: i32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupCatalog {
    #[serde(rename = "catalogID")]
    pub catalog_id: String,
    #[serde(rename = "catalogName")]
    pub catalog_name: String,
    #[serde(rename = "createTime")]
    pub create_time: String,
    #[serde(rename = "updateTime")]
    pub update_time: String,
    pub path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupContent {
    #[serde(rename = "contentID")]
    pub content_id: String,
    #[serde(rename = "contentName")]
    pub content_name: String,
    #[serde(rename = "contentSize")]
    pub content_size: i64,
    #[serde(rename = "createTime")]
    pub create_time: String,
    #[serde(rename = "updateTime")]
    pub update_time: String,
    #[serde(rename = "thumbnailURL")]
    pub thumbnail_url: Option<String>,
    pub digest: Option<String>,
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
pub struct BatchMoveResp {
    #[serde(flatten)]
    pub base: BaseResp,
}

#[derive(Debug, Deserialize)]
pub struct BatchCopyResp {
    #[serde(flatten)]
    pub base: BaseResp,
}

#[derive(Debug, Deserialize)]
pub struct BatchTrashResp {
    #[serde(flatten)]
    pub base: BaseResp,
}

#[derive(Debug, Serialize)]
pub struct ListRequest {
    #[serde(rename = "parentFileId")]
    pub parent_file_id: String,
    #[serde(rename = "pageNum")]
    pub page_num: i32,
    #[serde(rename = "pageSize")]
    pub page_size: i32,
    #[serde(rename = "orderBy")]
    pub order_by: Option<String>,
    #[serde(rename = "descending")]
    pub descending: Option<bool>,
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
pub struct FamilyListRequest {
    #[serde(rename = "catalogID")]
    pub catalog_id: String,
    #[serde(rename = "sortType")]
    pub sort_type: i32,
    #[serde(rename = "pageNumber")]
    pub page_number: i32,
    #[serde(rename = "pageSize")]
    pub page_size: i32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FamilyCreateFolderRequest {
    #[serde(rename = "catalogName")]
    pub catalog_name: String,
    #[serde(rename = "parentCatalogID")]
    pub parent_catalog_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupListRequest {
    #[serde(rename = "catalogID")]
    pub catalog_id: String,
    #[serde(rename = "sortType")]
    pub sort_type: i32,
    #[serde(rename = "pageNumber")]
    pub page_number: i32,
    #[serde(rename = "pageSize")]
    pub page_size: i32,
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
}

#[derive(Debug, Deserialize)]
pub struct BatchDeleteResp {
    #[serde(flatten)]
    pub base: BaseResp,
}
