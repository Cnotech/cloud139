use serde::Deserialize;

use super::common::BaseResp;

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
    #[serde(rename = "contentHash", default)]
    pub content_hash: Option<String>,
    #[serde(rename = "contentHashAlgorithm", default)]
    pub content_hash_algorithm: Option<String>,
}

impl PersonalFileItem {
    pub(crate) fn modified_time(&self) -> Option<&str> {
        self.updated_at
            .as_deref()
            .or(self.update_date.as_deref())
            .or(self.last_modified.as_deref())
    }

    pub fn is_folder(&self) -> bool {
        matches!(
            self.file_type.as_deref(),
            Some("folder") | Some("dir") | Some("1")
        )
    }
}

#[derive(Debug, Deserialize)]
pub struct PersonalThumbnail {
    pub style: Option<String>,
    pub url: Option<String>,
}
