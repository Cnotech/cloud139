#![allow(dead_code)]

mod delete_mock_test {
    use httpmock::prelude::*;
    use serde_json::json;

    #[test]
    fn test_mock_delete_personal() {
        let server = MockServer::start();

        let _m = server.mock(|when, then| {
            when.method(POST)
                .path("/recyclebin/batchTrash")
                .json_body(json!({"fileIds": ["123"]}));
            then.status(200).json_body(json!({
                "success": true,
                "message": "文件已移动到回收站"
            }));
        });
    }

    #[test]
    fn test_mock_delete_permanent() {
        let server = MockServer::start();

        let _m = server.mock(|when, then| {
            when.method(POST).path("/file/batchDelete");
            then.status(200).json_body(json!({
                "success": true,
                "message": "文件已彻底删除"
            }));
        });
    }

    #[test]
    fn test_mock_delete_family() {
        let server = MockServer::start();

        let _m = server.mock(|when, then| {
            when.method(POST)
                .path("/orchestration/familyCloud-rebuild/content/v1.0/deleteContent");
            then.status(200).json_body(json!({
                "success": true,
                "message": "删除成功"
            }));
        });
    }

    #[test]
    fn test_mock_delete_group() {
        let server = MockServer::start();

        let _m = server.mock(|when, then| {
            when.method(POST)
                .path("/orchestration/group-rebuild/content/v1.0/deleteGroupContent");
            then.status(200).json_body(json!({
                "success": true,
                "message": "删除成功"
            }));
        });
    }

    #[test]
    fn test_mock_delete_route_policy() {
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
