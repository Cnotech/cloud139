use super::client::{ListingRetry, PersonalClient, mutation_succeeded, require_success};
use crate::client::{ClientError, api::common::request::HttpClientWrapper};
use crate::config::Config;
use crate::debug;
use crate::protocol;

impl PersonalClient<'_> {
    pub(crate) async fn file_detail(
        &self,
        file_id: &str,
    ) -> Result<crate::models::QueryFileData, ClientError> {
        let response: crate::models::QueryFileResp = self
            .request(
                protocol::QUERY_FILE,
                &serde_json::json!({ "fileId": file_id }),
            )
            .await?;
        require_success(response.base, "获取文件详情失败")?;
        Ok(response.data)
    }

    pub(crate) async fn create_directory(
        &self,
        parent_id: &str,
        name: &str,
    ) -> Result<(), ClientError> {
        let response: crate::models::PersonalUploadResp = self
            .request(
                protocol::CREATE_FILE,
                &serde_json::json!({
                    "parentFileId": parent_id, "name": name, "description": "", "type": protocol::FILE_TYPE_FOLDER
                }),
            )
            .await?;
        require_success(response.base, "创建目录失败")
    }

    pub(crate) async fn rename(&self, file_id: &str, name: &str) -> Result<(), ClientError> {
        let response: crate::models::PersonalUploadResp = self
            .request(
                protocol::UPDATE_FILE,
                &serde_json::json!({ "fileId": file_id, "name": name, "description": "" }),
            )
            .await?;
        require_success(response.base, "重命名失败")
    }

    pub(crate) async fn trash(&self, file_id: &str) -> Result<(), ClientError> {
        let response: serde_json::Value = self
            .request(
                protocol::BATCH_TRASH,
                &serde_json::json!({ "fileIds": [file_id] }),
            )
            .await?;
        if !mutation_succeeded(&response) {
            return Err(ClientError::Api(
                response["message"]
                    .as_str()
                    .filter(|message| !message.is_empty())
                    .unwrap_or("删除失败：服务端未确认全部条目已移到回收站")
                    .to_owned(),
            ));
        }
        Ok(())
    }

    pub(crate) async fn move_files(
        &self,
        file_ids: &[String],
        target_id: &str,
    ) -> Result<(), ClientError> {
        self.transfer_files(protocol::BATCH_MOVE, file_ids, target_id)
            .await
    }

    pub(crate) async fn copy_files(
        &self,
        file_ids: &[String],
        target_id: &str,
    ) -> Result<(), ClientError> {
        self.transfer_files(protocol::BATCH_COPY, file_ids, target_id)
            .await
    }

    async fn transfer_files(
        &self,
        endpoint: &str,
        file_ids: &[String],
        target_id: &str,
    ) -> Result<(), ClientError> {
        let response: serde_json::Value = self
            .request(
                endpoint,
                &serde_json::json!({ "fileIds": file_ids, "toParentFileId": target_id }),
            )
            .await?;
        if !mutation_succeeded(&response) {
            return Err(ClientError::Api(
                "批量操作失败（可能部分完成），请检查云端状态".into(),
            ));
        }
        Ok(())
    }
}

pub async fn get_personal_file_detail(
    config: &Config,
    file_id: &str,
) -> Result<crate::models::QueryFileData, ClientError> {
    debug!("查询个人云文件详情 file_id={}", file_id);
    let client = HttpClientWrapper::new();
    let personal = PersonalClient::connect(config, &client, ListingRetry::None).await?;
    personal.file_detail(file_id).await
}
