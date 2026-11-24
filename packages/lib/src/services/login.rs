//! 验证登录 Token 并生成个人云配置。

use super::list::{ListOptions, list_personal};
use crate::client::http::HttpClientWrapper;
use crate::config::{Config, personal::PersonalConfig};
use crate::{info, protocol};

/// 解析登录 Token，通过读取云端根目录验证凭据，再返回配置。
///
/// 调用方可用 [`Config::save_to`] 将配置保存到指定路径。
pub async fn login(token: &str) -> anyhow::Result<Config> {
    login_with_route_url(token, protocol::ROUTE_POLICY_URL).await
}

async fn login_with_route_url(token: &str, route_url: &str) -> anyhow::Result<Config> {
    let mut config = PersonalConfig::new(crate::client::auth::token::parse_login_token(token)?)?;
    info!("正在校验 Token 可用性 (ls /) ...");
    let host = HttpClientWrapper::new()
        .resolve_personal_cloud_host_at(config.as_ref(), route_url)
        .await?;
    config.set_cloud_host(host);
    let options = ListOptions {
        path: "/",
        page: 1,
        page_size: 10,
    };
    list_personal(config.as_ref(), options)
        .await
        .map_err(|error| anyhow::anyhow!("登录校验失败: {error}"))?;
    Ok(config.into_inner())
}

#[cfg(test)]
mod tests {
    use super::{login, login_with_route_url};
    use crate::error::ClientError;
    use base64::{Engine, engine::general_purpose::STANDARD};
    use httpmock::{Mock, prelude::*};
    use serde_json::json;

    const ROUTE_PATH: &str = "/user/route/qryRoutePolicy";
    const ACCOUNT: &str = "test-account";
    const TOKEN_INFO: &str = "mock|refresh|token|1800000000000";

    fn test_token() -> String {
        STANDARD.encode(format!("mock:{ACCOUNT}:{TOKEN_INFO}"))
    }

    fn mock_route(server: &MockServer) -> Mock<'_> {
        server.mock(|when, then| {
            when.method(POST)
                .path(ROUTE_PATH)
                .json_body_includes(json!({"userInfo": {"accountName": ACCOUNT}}).to_string());
            then.json_body(json!({
                "success": true,
                "code": "0000",
                "message": "",
                "data": {
                    "routePolicyList": [
                        {"modName": "personal", "httpsUrl": server.base_url()}
                    ]
                }
            }));
        })
    }

    #[tokio::test]
    async fn login_rejects_invalid_token() {
        let error = login("!").await.unwrap_err();
        assert!(matches!(
            error.downcast_ref::<ClientError>(),
            Some(ClientError::Other(message)) if message == "Token 格式错误，无法解析"
        ));
    }

    #[tokio::test]
    async fn login_propagates_route_failure() {
        let server = MockServer::start();
        let route = server.mock(|when, then| {
            when.method(POST).path(ROUTE_PATH);
            then.status(503);
        });

        let error = login_with_route_url(&test_token(), &server.url(ROUTE_PATH))
            .await
            .unwrap_err();
        assert!(matches!(
            error.downcast_ref::<ClientError>(),
            Some(ClientError::Http(error)) if error.status() == Some(reqwest::StatusCode::SERVICE_UNAVAILABLE)
        ));
        route.assert_calls(1);
    }

    #[tokio::test]
    async fn login_rejects_failed_root_listing() {
        let server = MockServer::start();
        let route = mock_route(&server);
        let root = server.mock(|when, then| {
            when.method(POST)
                .path("/file/list")
                .json_body_includes(r#"{"parentFileId":"/"}"#);
            then.json_body(json!({"success": false, "message": "模拟服务端拒绝凭据"}));
        });

        let error = login_with_route_url(&test_token(), &server.url(ROUTE_PATH))
            .await
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            "登录校验失败: API 错误: 模拟服务端拒绝凭据"
        );
        route.assert_calls(1);
        root.assert_calls(1);
    }

    #[tokio::test]
    async fn login_returns_resolved_config_after_root_listing() {
        let server = MockServer::start();
        let token = test_token();
        let route = mock_route(&server);
        let root = server.mock(|when, then| {
            when.method(POST)
                .path("/file/list")
                .header("Authorization", format!("Basic {token}"))
                .json_body_includes(
                    json!({
                        "parentFileId": "/",
                        "pageInfo": {"pageCursor": "", "pageSize": 10}
                    })
                    .to_string(),
                );
            then.json_body(json!({
                "success": true,
                "data": {"items": [], "nextPageCursor": ""}
            }));
        });

        let config = login_with_route_url(&token, &server.url(ROUTE_PATH))
            .await
            .unwrap();
        assert_eq!(config.account, ACCOUNT);
        assert_eq!(config.authorization, token);
        assert_eq!(config.refresh_token.as_deref(), Some(TOKEN_INFO));
        assert_eq!(config.token_expire_time, Some(1_800_000_000_000));
        assert_eq!(config.personal_cloud_host, Some(server.base_url()));
        route.assert_calls(1);
        root.assert_calls(1);
    }
}
