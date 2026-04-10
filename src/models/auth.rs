use serde::Deserialize;

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
    #[serde(rename = "siteID", default)]
    pub site_id: Option<String>,
    #[serde(rename = "siteCode", default)]
    pub site_code: Option<String>,
    #[serde(rename = "modName", default)]
    pub mod_name: Option<String>,
    #[serde(rename = "httpUrl", default)]
    pub http_url: Option<String>,
    #[serde(rename = "httpsUrl", default)]
    pub https_url: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename = "root")]
pub struct RefreshTokenResp {
    #[serde(rename = "return", default)]
    pub return_code: Option<String>,
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub expiretime: Option<i32>,
    #[serde(rename = "accessToken", default)]
    pub access_token: Option<String>,
    #[serde(default)]
    pub desc: Option<String>,
}
