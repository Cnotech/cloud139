#![allow(dead_code)]

mod download_mock_test {
    use httpmock::prelude::*;
    use serde_json::json;

    #[test]
    fn test_mock_download_personal_url() {
        let server = MockServer::start();

        let _m = server.mock(|when, then| {
            when.method(POST)
                .path("/file/getDownloadUrl")
                .json_body(json!({"fileId": "123"}));
            then.status(200).json_body(json!({
                "success": true,
                "data": {
                    "url": "http://example.com/file",
                    "cdnUrl": "http://cdn.example.com/file"
                }
            }));
        });
    }

    #[test]
    fn test_mock_download_family_url() {
        let server = MockServer::start();

        let _m = server.mock(|when, then| {
            when.method(POST)
                .path("/orchestration/familyCloud-rebuild/content/v1.0/getFileDownLoadURL");
            then.status(200).json_body(json!({
                "success": true,
                "data": {
                    "downloadURL": "http://family.example.com/download"
                }
            }));
        });
    }

    #[test]
    fn test_mock_download_group_url() {
        let server = MockServer::start();

        let _m = server.mock(|when, then| {
            when.method(POST)
                .path("/orchestration/group-rebuild/groupManage/v1.0/getGroupFileDownLoadURL");
            then.status(200).json_body(json!({
                "success": true,
                "data": {
                    "downloadURL": "http://group.example.com/download"
                }
            }));
        });
    }

    #[test]
    fn test_mock_download_route_policy() {
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

mod download_path_tests {
    use cloud139::commands::download::resolve_local_path;

    #[test]
    fn test_resolve_local_path_with_directory() {
        let result = resolve_local_path("/remote/path/file.txt", &Some("/local/dir/".to_string()));
        assert!(result.ends_with("file.txt"));
        assert!(result.starts_with("/local/dir/"));
    }

    #[test]
    fn test_resolve_local_path_no_local_path() {
        let result = resolve_local_path("/remote/path/myfile.zip", &None);
        assert_eq!(result, "myfile.zip");
    }

    #[test]
    fn test_resolve_local_path_explicit_file() {
        let result = resolve_local_path(
            "/remote/path/original.txt",
            &Some("/local/renamed.txt".to_string()),
        );
        assert_eq!(result, "/local/renamed.txt");
    }

    #[test]
    fn test_resolve_local_path_empty_remote() {
        let result = resolve_local_path("", &None);
        assert_eq!(result, "download");
    }
}
