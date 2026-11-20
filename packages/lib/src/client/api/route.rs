use crate::client::{api::headers::build_route_headers, http::HttpClientWrapper};
use crate::error::ClientError;
use crate::models::route::{QueryRoutePolicyResp, RoutePolicy};
use crate::protocol;

pub(super) async fn query_route_policy(
    client: &HttpClientWrapper,
    account: &str,
    account_type: i32,
    authorization: &str,
) -> Result<Vec<RoutePolicy>, ClientError> {
    let body = serde_json::json!({
        "userInfo": {
            "userType": protocol::ROUTE_USER_TYPE,
            "accountType": account_type,
            "accountName": account
        },
        "modAddrType": protocol::ROUTE_ADDRESS_TYPE
    });
    let response: QueryRoutePolicyResp = client
        .signed_post(
            authorization,
            protocol::ROUTE_POLICY_URL,
            "/user/route/qryRoutePolicy",
            &body,
            build_route_headers,
        )
        .await?;
    Ok(response.data.route_policy_list)
}
