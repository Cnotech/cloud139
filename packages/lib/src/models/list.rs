use chrono::NaiveDateTime;
use serde::Deserialize;

use super::common::BaseResp;
use crate::protocol;
use crate::utils::crypto::is_sha256_hex;

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

#[derive(Debug, Clone, Deserialize)]
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

    pub(crate) fn content_sha256(&self) -> Option<String> {
        let hash = self.content_hash.as_deref()?.trim();
        let algorithm = self.content_hash_algorithm.as_deref()?.trim();
        let is_sha256 = algorithm.eq_ignore_ascii_case(protocol::CONTENT_HASH_SHA256)
            || algorithm.eq_ignore_ascii_case(protocol::CONTENT_HASH_SHA256_ALIAS);
        (is_sha256 && is_sha256_hex(hash)).then(|| hash.to_ascii_lowercase())
    }

    pub fn is_folder(&self) -> bool {
        matches!(
            self.file_type.as_deref(),
            Some(protocol::FILE_TYPE_FOLDER)
                | Some(protocol::FILE_TYPE_DIR)
                | Some(protocol::FILE_TYPE_DIRECTORY_CODE)
        )
    }
}

/// 将个人云列表中的时间统一为日期和时分秒。
pub fn parse_personal_time(time_str: &str) -> String {
    if time_str.is_empty() {
        return String::new();
    }
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(time_str) {
        return dt.format("%Y-%m-%d %H:%M:%S").to_string();
    }
    if let Ok(dt) = NaiveDateTime::parse_from_str(time_str, "%Y-%m-%dT%H:%M:%S%.f") {
        return dt.format("%Y-%m-%d %H:%M:%S").to_string();
    }
    time_str.to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub struct PersonalThumbnail {
    pub style: Option<String>,
    pub url: Option<String>,
}
