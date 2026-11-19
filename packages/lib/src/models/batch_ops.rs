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
