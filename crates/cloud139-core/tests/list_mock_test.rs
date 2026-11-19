#![allow(dead_code)]

mod list_mock_test {
    use httpmock::prelude::*;
    use serde_json::json;

    #[test]
    fn test_mock_list_personal_root() {
        let server = MockServer::start();

        let _m = server.mock(|when, then| {
            when.method(POST).path("/file/list").json_body(json!({
                "parentFileId": "/",
                "pageInfo": {"pageCursor": "", "pageSize": 100}
            }));
            then.status(200).json_body(json!({
                "success": true,
                "data": {
                    "items": [
                        {"fileId": "123", "name": "test.txt", "size": 1024, "type": "file"}
                    ]
                }
            }));
        });
    }

    #[test]
    fn test_mock_list_with_pagination() {
        let server = MockServer::start();

        let _m = server.mock(|when, then| {
            when.method(POST).path("/file/list").json_body(json!({
                "parentFileId": "/",
                "pageInfo": {"pageCursor": "cursor1", "pageSize": 10}
            }));
            then.status(200).json_body(json!({
                "success": true,
                "data": {
                    "items": [],
                    "nextPageCursor": ""
                }
            }));
        });
    }

    #[test]
    fn test_mock_list_family() {
        let server = MockServer::start();

        let _m = server.mock(|when, then| {
            when.method(POST)
                .path("/orchestration/familyCloud-rebuild/content/v1.2/queryContentList");
            then.status(200).json_body(json!({
                "success": true,
                "data": {
                    "result": {"resultCode": "0"},
                    "cloudContentList": [],
                    "cloudCatalogList": []
                }
            }));
        });
    }

    #[test]
    fn test_mock_list_group() {
        let server = MockServer::start();

        let _m = server.mock(|when, then| {
            when.method(POST)
                .path("/orchestration/group-rebuild/content/v1.0/queryGroupContentList");
            then.status(200).json_body(json!({
                "success": true,
                "data": {
                    "result": {"resultCode": "0"},
                    "getGroupContentResult": {
                        "catalogList": [],
                        "contentList": []
                    }
                }
            }));
        });
    }

    #[test]
    fn test_mock_list_route_policy() {
        let server = MockServer::start();

        let _m = server.mock(|when, then| {
            when.method(POST).path("/user/route/qryRoutePolicy");
            then.status(200).json_body(json!({
                "success": true,
                "data": {
                    "routePolicyList": [
                        {"modName": "personal", "httpsUrl": server.url("/")}
                    ]
                }
            }));
        });
    }
}
