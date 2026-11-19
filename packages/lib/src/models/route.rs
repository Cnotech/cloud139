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
