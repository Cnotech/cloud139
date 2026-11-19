use super::{headers::build_signed_headers, request::HttpClientWrapper};
use crate::client::ClientError;
use crate::debug;
use crate::models::{QueryRoutePolicyResp, RoutePolicy};
use crate::protocol;
use reqwest::header::HeaderMap;

pub(in crate::client::api) async fn query_route_policy(
    client: &HttpClientWrapper,
    account: &str,
    account_type: i32,
    authorization: &str,
) -> Result<Vec<RoutePolicy>, ClientError> {
    let url = protocol::ROUTE_POLICY_URL;
    debug!("查询云盘路由 url={}", url);
    let body = serde_json::json!({
        "userInfo": {
            "userType": protocol::ROUTE_USER_TYPE,
            "accountType": account_type,
            "accountName": account
        },
        "modAddrType": protocol::ROUTE_ADDRESS_TYPE
    });
    let response: QueryRoutePolicyResp = client
        .signed_post(authorization, url, &body, build_route_headers)
        .await?;
    Ok(response.data.route_policy_list)
}

pub fn build_route_headers(
    authorization: &str,
    ts: &str,
    rand_str: &str,
    sign: &str,
) -> Result<HeaderMap, ClientError> {
    build_signed_headers(protocol::ROUTE_HEADERS, authorization, ts, rand_str, sign)
}
