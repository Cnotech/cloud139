use serde::{Deserialize, Serialize};

use super::common::{ApiResult, BaseResp};

#[derive(Debug, Deserialize)]
pub struct PersonalListResp {
    #[serde(flatten)]
    pub base: BaseResp,
    #[serde(default)]
    pub data: Option<PersonalListData>,
}

#[derive(Debug, Deserialize)]
pub struct PersonalListData {
    pub items: Vec<PersonalFileItem>,
    #[serde(rename = "nextPageCursor", default)]
    pub next_page_cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PersonalFileItem {
    #[serde(rename = "fileId", default)]
    pub file_id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub size: Option<i64>,
    #[serde(rename = "type", default)]
    pub file_type: Option<String>,
    #[serde(rename = "createdAt", default)]
    pub created_at: Option<String>,
    #[serde(rename = "updatedAt", default)]
    pub updated_at: Option<String>,
    #[serde(rename = "createDate", default)]
    pub create_date: Option<String>,
    #[serde(rename = "updateDate", default)]
    pub update_date: Option<String>,
    #[serde(rename = "lastModified", default)]
    pub last_modified: Option<String>,
    #[serde(rename = "thumbnailUrls", default)]
    pub thumbnail_urls: Option<Vec<PersonalThumbnail>>,
}

#[derive(Debug, Deserialize)]
pub struct PersonalThumbnail {
    pub style: Option<String>,
    pub url: Option<String>,
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FamilyListRequest {
    #[serde(rename = "catalogID")]
    pub catalog_id: String,
    #[serde(rename = "contentSortType")]
    pub content_sort_type: i32,
    #[serde(rename = "sortDirection")]
    pub sort_direction: i32,
    #[serde(rename = "pageInfo")]
    pub page_info: PageInfo,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageInfo {
    #[serde(rename = "pageNum")]
    pub page_num: i32,
    #[serde(rename = "pageSize")]
    pub page_size: i32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupListRequest {
    #[serde(rename = "groupID")]
    pub group_id: String,
    #[serde(rename = "catalogID")]
    pub catalog_id: String,
    #[serde(rename = "contentSortType")]
    pub content_sort_type: i32,
    #[serde(rename = "sortDirection")]
    pub sort_direction: i32,
    #[serde(rename = "startNumber")]
    pub start_number: i32,
    #[serde(rename = "endNumber")]
    pub end_number: i32,
    pub path: String,
}
