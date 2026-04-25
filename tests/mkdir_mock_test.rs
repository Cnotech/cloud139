#![allow(dead_code)]

mod mkdir_mock_test {
    use cloud139::application::services::mkdir_service::parse_path;
    use httpmock::prelude::*;
    use serde_json::json;

    #[test]
    fn test_parse_path_root() {
        let result = parse_path("/");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_path_single_level() {
        let result = parse_path("test");
        assert!(result.is_ok());
        let (parent, name) = result.unwrap();
        assert_eq!(parent, "/");
        assert_eq!(name, "test");
    }

    #[test]
    fn test_parse_path_multi_level() {
        let result = parse_path("/folder1/folder2");
        assert!(result.is_ok());
        let (parent, name) = result.unwrap();
        assert_eq!(parent, "/folder1");
        assert_eq!(name, "folder2");
    }

    #[test]
    fn test_parse_path_with_leading_slash() {
        let result = parse_path("/test");
        assert!(result.is_ok());
        let (parent, name) = result.unwrap();
        assert_eq!(parent, "/");
        assert_eq!(name, "test");
    }

    #[test]
    fn test_parse_path_empty() {
        let result = parse_path("");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_path_with_spaces() {
        let result = parse_path("  /folder/name  ");
        assert!(result.is_ok());
        let (parent, name) = result.unwrap();
        assert_eq!(parent, "/folder");
        assert_eq!(name, "name");
    }

    #[test]
    fn test_mock_mkdir_personal_endpoint() {
        let server = MockServer::start();

        let _m = server.mock(|when, then| {
            when.method(POST).path("/file/create").json_body(json!({
                "parentFileId": "parent123",
                "name": "test_folder",
                "type": "folder"
            }));
            then.status(200).json_body(json!({
                "success": true,
                "data": {
                    "fileId": "123456",
                    "fileName": "test_folder"
                }
            }));
        });
    }

    #[test]
    fn test_mock_mkdir_route_policy() {
        let server = MockServer::start();

        let _m = server.mock(|when, then| {
            when.method(POST).path("/user/route/qryRoutePolicy");
            then.status(200).json_body(json!({
                "success": true,
                "data": {
                    "routePolicyList": [
                        {"modName": "personal", "httpsUrl": "https://personal.cloud.139.com"}
                    ]
                }
            }));
        });
    }

    #[test]
    fn test_mock_mkdir_family_endpoint() {
        let server = MockServer::start();

        let _m = server.mock(|when, then| {
            when.method(POST)
                .path("/orchestration/familyCloud-rebuild/cloudCatalog/v1.0/createCloudDoc");
            then.status(200).json_body(json!({
                "success": true,
                "message": "创建成功"
            }));
        });
    }

    #[test]
    fn test_mock_mkdir_group_endpoint() {
        let server = MockServer::start();

        let _m = server.mock(|when, then| {
            when.method(POST)
                .path("/orchestration/group-rebuild/catalog/v1.0/createGroupCatalog");
            then.status(200).json_body(json!({
                "success": true,
                "message": "创建成功"
            }));
        });
    }
}
