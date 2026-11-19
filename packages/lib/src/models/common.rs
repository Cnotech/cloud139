use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct BaseResp {
    pub success: bool,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CommonAccountInfo {
    pub account: String,
    #[serde(rename = "accountType")]
    pub account_type: i32,
}
