#![allow(dead_code)]

mod cp_mock_test {
    use httpmock::prelude::*;
    use serde_json::json;

    #[test]
    fn test_mock_cp_personal() {
        let server = MockServer::start();

        let _m = server.mock(|when, then| {
            when.method(POST).path("/file/batchCopy").json_body(json!({
                "fileIds": ["123"],
                "targetParentFileId": "456"
            }));
            then.status(200).json_body(json!({
                "success": true,
                "message": "复制成功"
            }));
        });
    }

    #[test]
    fn test_mock_cp_family() {
        let server = MockServer::start();

        let _m = server.mock(|when, then| {
            when.method(POST)
                .path("/orchestration/familyCloud-rebuild/content/v1.0/copyContent");
            then.status(200).json_body(json!({
                "success": true,
                "message": "复制成功"
            }));
        });
    }

    #[test]
    fn test_mock_cp_group() {
        let server = MockServer::start();

        let _m = server.mock(|when, then| {
            when.method(POST)
                .path("/orchestration/group-rebuild/content/v1.0/copyGroupContent");
            then.status(200).json_body(json!({
                "success": true,
                "message": "复制成功"
            }));
        });
    }

    #[test]
    fn test_mock_cp_route_policy() {
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
