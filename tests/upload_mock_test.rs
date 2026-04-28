#![allow(dead_code)]

mod upload_mock_test {
    use cloud139::utils::resolve_local_path;
    use httpmock::prelude::*;
    use serde_json::json;

    #[test]
    fn test_mock_upload_init() {
        let server = MockServer::start();

        let _m = server.mock(|when, then| {
            when.method(POST).path("/file/uploadInit");
            then.status(200).json_body(json!({
                "success": true,
                "data": {
                    "fileId": "123456",
                    "fileName": "test.txt",
                    "rapidUpload": true
                }
            }));
        });
    }

    #[test]
    fn test_mock_upload_init_multipart() {
        let server = MockServer::start();

        let _m = server.mock(|when, then| {
            when.method(POST).path("/file/uploadInit");
            then.status(200).json_body(json!({
                "success": true,
                "data": {
                    "fileId": "123456",
                    "fileName": "large.txt",
                    "rapidUpload": false,
                    "uploadId": "upload_abc",
                    "partInfos": [
                        {"partNumber": 1, "uploadUrl": "http://upload1.example.com"},
                        {"partNumber": 2, "uploadUrl": "http://upload2.example.com"}
                    ]
                }
            }));
        });
    }

    #[test]
    fn test_mock_upload_complete() {
        let server = MockServer::start();

        let _m = server.mock(|when, then| {
            when.method(POST).path("/file/uploadComplete");
            then.status(200).json_body(json!({
                "success": true,
                "message": "上传成功"
            }));
        });
    }

    #[test]
    fn test_mock_upload_family() {
        let server = MockServer::start();

        let _m = server.mock(|when, then| {
            when.method(POST)
                .path("/orchestration/familyCloud-rebuild/content/v1.0/uploadFile");
            then.status(200).json_body(json!({
                "success": true,
                "message": "上传成功"
            }));
        });
    }

    #[test]
    fn test_mock_upload_group() {
        let server = MockServer::start();

        let _m = server.mock(|when, then| {
            when.method(POST)
                .path("/orchestration/group-rebuild/content/v1.0/uploadGroupFile");
            then.status(200).json_body(json!({
                "success": true,
                "message": "上传成功"
            }));
        });
    }

    #[test]
    fn test_mock_upload_route_policy() {
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

    #[test]
    fn test_resolve_local_path_basic() {
        let _result = resolve_local_path("test.txt", &None);
        let _result2 = resolve_local_path("folder/test.txt", &None);
        let _result3 = resolve_local_path("test.txt", &Some("output.txt".to_string()));
        let _result4 = resolve_local_path("test.txt", &Some("downloads".to_string()));
    }
}

mod upload_part_size_tests {
    use cloud139::services::upload_service::get_part_size;

    const MB: i64 = 1024 * 1024;
    const GB: i64 = 1024 * MB;

    #[test]
    fn test_get_part_size_default_small_file() {
        let size = get_part_size(10 * GB, 0);
        assert_eq!(size, 100 * MB, "小文件默认分片应为 100MB");
    }

    #[test]
    fn test_get_part_size_default_large_file() {
        let size = get_part_size(31 * GB, 0);
        assert_eq!(size, 512 * MB, "大文件默认分片应为 512MB");
    }

    #[test]
    fn test_get_part_size_custom() {
        let custom = 256 * MB;
        let size = get_part_size(100 * GB, custom);
        assert_eq!(size, custom, "自定义分片大小应原样返回");
    }
}
