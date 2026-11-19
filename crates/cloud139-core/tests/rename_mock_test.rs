#![allow(dead_code)]

mod rename_mock_test {
    use httpmock::prelude::*;
    use serde_json::json;

    #[test]
    fn test_mock_rename_personal() {
        let server = MockServer::start();

        let _m = server.mock(|when, then| {
            when.method(POST)
                .path("/file/batchRename")
                .json_body(json!({
                    "fileIds": ["123"],
                    "newNames": ["new_name.txt"]
                }));
            then.status(200).json_body(json!({
                "success": true,
                "message": "重命名成功"
            }));
        });
    }

    #[test]
    fn test_mock_rename_family() {
        let server = MockServer::start();

        let _m = server.mock(|when, then| {
            when.method(POST)
                .path("/orchestration/familyCloud-rebuild/cloudCatalog/v1.0/renameCloudDoc");
            then.status(200).json_body(json!({
                "success": true,
                "message": "重命名成功"
            }));
        });
    }

    #[test]
    fn test_mock_rename_group() {
        let server = MockServer::start();

        let _m = server.mock(|when, then| {
            when.method(POST)
                .path("/orchestration/group-rebuild/catalog/v1.0/renameGroupCatalog");
            then.status(200).json_body(json!({
                "success": true,
                "message": "重命名成功"
            }));
        });
    }

    #[test]
    fn test_mock_rename_route_policy() {
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
