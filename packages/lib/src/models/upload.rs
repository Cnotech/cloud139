use serde::{Deserialize, Serialize};

use super::common::{BaseResp, CommonAccountInfo};
use crate::error::ClientError;

/// 上传请求和分片读取共用的布局。
#[derive(Clone, Copy)]
pub(crate) struct UploadLayout {
    file_size: i64,
    part_size: i64,
    part_count: i64,
}

impl UploadLayout {
    pub(crate) fn new(file_size: i64, part_size: i64) -> Result<Self, ClientError> {
        if file_size < 0 || part_size <= 0 {
            return Err(ClientError::Api(
                "文件大小必须非负，上传分片大小必须大于 0".into(),
            ));
        }
        let part_count = file_size / part_size + i64::from(file_size % part_size != 0);
        if part_count > i32::MAX as i64 {
            return Err(ClientError::Api("上传分片数量超出协议范围".into()));
        }
        Ok(Self {
            file_size,
            part_size,
            part_count,
        })
    }

    pub(crate) fn file_size(self) -> i64 {
        self.file_size
    }

    pub(crate) fn part_count(self) -> i64 {
        self.part_count
    }

    pub(crate) fn part_offset_and_size(self, index: i64) -> (i64, i64) {
        let offset = index * self.part_size;
        (offset, self.part_size.min(self.file_size - offset))
    }
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

#[cfg(test)]
mod tests {
    use super::UploadLayout;

    #[test]
    fn part_count_handles_large_sizes_and_rejects_invalid_layouts() {
        assert_eq!(
            UploadLayout::new(i64::MAX, i64::MAX).unwrap().part_count(),
            1
        );
        assert_eq!(UploadLayout::new(0, 100).unwrap().part_count(), 0);
        assert_eq!(UploadLayout::new(101, 100).unwrap().part_count(), 2);
        for (size, part) in [(1, 0), (1, -1), (-1, 100), (i64::MAX, 1)] {
            assert!(UploadLayout::new(size, part).is_err());
        }
    }
}
