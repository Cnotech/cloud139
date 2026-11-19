use serde::Deserialize;

use super::common::BaseResp;

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

#[derive(Debug, Deserialize)]
pub struct BatchRenameResp {
    #[serde(flatten)]
    pub base: BaseResp,
}

#[derive(Debug, Deserialize)]
pub struct BatchDeleteResp {
    #[serde(flatten)]
    pub base: BaseResp,
}

#[derive(Debug, Deserialize)]
pub struct ApiResult {
    #[serde(rename = "resultCode")]
    pub result_code: String,
    #[serde(rename = "resultDesc")]
    pub result_desc: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateBatchOprTaskResp {
    pub result: ApiResult,
    #[serde(rename = "taskID")]
    pub task_id: String,
}
