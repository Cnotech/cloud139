#![allow(dead_code)]

mod api_extended_test {
    use cloud139::client::StorageType;
    use cloud139::client::api::{
        HttpClientWrapper, check_file_exists_with_client, get_parent_id,
        get_personal_cloud_host_with_client, list_personal_files_with_client, parse_path_segments,
    };
    use cloud139::config::Config;
    use cloud139::models::*;

    #[test]
    fn test_parse_path_segments_empty() {
        let result = parse_path_segments("");
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_path_segments_root() {
        let result = parse_path_segments("/");
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_path_segments_single() {
        let result = parse_path_segments("test");
        assert_eq!(result, vec!["test"]);
    }

    #[test]
    fn test_parse_path_segments_multiple() {
        let result = parse_path_segments("/folder1/folder2/file.txt");
        assert_eq!(result, vec!["folder1", "folder2", "file.txt"]);
    }

    #[test]
    fn test_parse_path_segments_with_leading_slash() {
        let result = parse_path_segments("/folder1/folder2");
        assert_eq!(result, vec!["folder1", "folder2"]);
    }

    #[test]
    fn test_parse_path_segments_multiple_slashes() {
        let result = parse_path_segments("///folder///file///");
        assert_eq!(result, vec!["folder", "file"]);
    }

    #[test]
    fn test_get_parent_id_empty() {
        let result = get_parent_id("");
        assert_eq!(result, "/");
    }

    #[test]
    fn test_get_parent_id_root() {
        let result = get_parent_id("/");
        assert_eq!(result, "/");
    }

    #[test]
    fn test_get_parent_id_with_value() {
        let result = get_parent_id("12345");
        assert_eq!(result, "12345");
    }

    #[test]
    fn test_http_client_wrapper_default() {
        let wrapper = HttpClientWrapper::default();
        assert!(std::mem::size_of_val(&wrapper.client) > 0);
    }

    #[test]
    fn test_http_client_wrapper_new() {
        let wrapper = HttpClientWrapper::new();
        assert!(std::mem::size_of_val(&wrapper.client) > 0);
    }

    #[test]
    fn test_storage_type_svc_type() {
        assert_eq!(StorageType::PersonalNew.svc_type(), "1");
        assert_eq!(StorageType::Family.svc_type(), "2");
        assert_eq!(StorageType::Group.svc_type(), "3");
    }

    #[test]
    fn test_storage_type_as_str() {
        assert_eq!(StorageType::PersonalNew.as_str(), "personal_new");
        assert_eq!(StorageType::Family.as_str(), "family");
        assert_eq!(StorageType::Group.as_str(), "group");
    }

    #[test]
    fn test_storage_type_from_str() {
        assert_eq!(StorageType::from_str_raw("family"), StorageType::Family);
        assert_eq!(StorageType::from_str_raw("group"), StorageType::Group);
        assert_eq!(
            StorageType::from_str_raw("personal_new"),
            StorageType::PersonalNew
        );
        assert_eq!(
            StorageType::from_str_raw("unknown"),
            StorageType::PersonalNew
        );
    }

    #[tokio::test]
    async fn test_get_personal_cloud_host_with_cached_host() {
        let mut config = Config::default();
        config.personal_cloud_host = Some("https://cached.example.com".to_string());

        let wrapper = HttpClientWrapper::new();
        let result = get_personal_cloud_host_with_client(&mut config, &wrapper).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "https://cached.example.com".to_string());
    }

    #[tokio::test]
    async fn test_get_personal_cloud_host_empty_account() {
        let mut config = Config::default();
        config.account = "".to_string();
        config.authorization = "test".to_string();

        let wrapper = HttpClientWrapper::new();
        let result = get_personal_cloud_host_with_client(&mut config, &wrapper).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_file_id_by_path_root() {
        let config = Config::default();
        let result = cloud139::client::api::get_file_id_by_path(&config, "/").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "");
    }

    #[tokio::test]
    async fn test_get_file_id_by_path_empty() {
        let config = Config::default();
        let result = cloud139::client::api::get_file_id_by_path(&config, "").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "");
    }

    #[tokio::test]
    async fn test_list_personal_files_empty_parent() {
        let mut config = Config::default();
        config.personal_cloud_host = Some("https://test.example.com".to_string());
        config.account = "test@139.com".to_string();
        config.authorization = "Basic dGVzdA==".to_string();

        let wrapper = HttpClientWrapper::new();
        let result = list_personal_files_with_client(&config, "/", &wrapper).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_check_file_exists_empty_parent() {
        let mut config = Config::default();
        config.personal_cloud_host = Some("https://test.example.com".to_string());
        config.account = "test@139.com".to_string();
        config.authorization = "Basic dGVzdA==".to_string();

        let wrapper = HttpClientWrapper::new();
        let result = check_file_exists_with_client(&config, "/", "test.txt", &wrapper).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_base_resp_with_success() {
        let json = r#"{"success": true}"#;
        let resp: BaseResp = serde_json::from_str(json).unwrap();
        assert!(resp.success);
    }

    #[test]
    fn test_api_result_deserialize() {
        let json = r#"{"resultCode": "0", "resultDesc": "success"}"#;
        let result: ApiResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.result_code, "0");
        assert_eq!(result.result_desc, Some("success".to_string()));
    }

    #[test]
    fn test_personal_list_resp_full() {
        let json = r#"{
            "success": true,
            "data": {
                "items": [
                    {
                        "fileId": "123",
                        "name": "test.txt",
                        "size": 1024,
                        "type": "file"
                    }
                ],
                "nextPageCursor": "abc"
            }
        }"#;
        let resp: PersonalListResp = serde_json::from_str(json).unwrap();
        assert!(resp.base.success);
        let data = resp.data.unwrap();
        assert_eq!(data.items.len(), 1);
        assert_eq!(data.next_page_cursor, Some("abc".to_string()));
    }

    #[test]
    fn test_personal_list_resp_no_data() {
        let json = r#"{"success": true}"#;
        let resp: PersonalListResp = serde_json::from_str(json).unwrap();
        assert!(resp.base.success);
        assert!(resp.data.is_none());
    }

    #[test]
    fn test_download_url_resp_with_cdn() {
        let json = r#"{
            "success": true,
            "data": {
                "url": "http://example.com/file",
                "cdnUrl": "http://cdn.example.com/file",
                "fileName": "test.txt"
            }
        }"#;
        let resp: DownloadUrlResp = serde_json::from_str(json).unwrap();
        assert!(resp.base.success);
        assert_eq!(
            resp.data.cdn_url,
            Some("http://cdn.example.com/file".to_string())
        );
    }

    #[test]
    fn test_download_url_resp_without_cdn() {
        let json = r#"{
            "success": true,
            "data": {
                "url": "http://example.com/file"
            }
        }"#;
        let resp: DownloadUrlResp = serde_json::from_str(json).unwrap();
        assert!(resp.base.success);
        assert_eq!(resp.data.url, Some("http://example.com/file".to_string()));
    }

    #[test]
    fn test_query_route_policy_resp_parse() {
        let json = r#"{
            "success": true,
            "code": "0",
            "message": "",
            "data": {
                "routePolicyList": [
                    {
                        "modName": "personal",
                        "httpsUrl": "https://personal.cloud.139.com"
                    },
                    {
                        "modName": "family",
                        "httpsUrl": "https://family.cloud.139.com"
                    }
                ]
            }
        }"#;
        let resp: QueryRoutePolicyResp = serde_json::from_str(json).unwrap();
        assert!(resp.success);
        assert_eq!(resp.data.route_policy_list.len(), 2);
    }

    #[test]
    fn test_personal_file_item_all_fields() {
        let json = r#"{
            "fileId": "123",
            "name": "test.txt",
            "size": 1024,
            "type": "file",
            "createdAt": "2024-01-01T00:00:00Z",
            "updatedAt": "2024-01-02T00:00:00Z",
            "createDate": "2024-01-01",
            "updateDate": "2024-01-02",
            "lastModified": "2024-01-03"
        }"#;
        let item: PersonalFileItem = serde_json::from_str(json).unwrap();
        assert_eq!(item.file_id, Some("123".to_string()));
        assert_eq!(item.name, Some("test.txt".to_string()));
        assert_eq!(item.size, Some(1024));
        assert_eq!(item.file_type, Some("file".to_string()));
    }

    #[test]
    fn test_personal_upload_resp_rapid_upload() {
        let json = r#"{
            "success": true,
            "data": {
                "fileId": "123",
                "fileName": "test.txt",
                "rapidUpload": true
            }
        }"#;
        let resp: PersonalUploadResp = serde_json::from_str(json).unwrap();
        assert!(resp.base.success);
        assert_eq!(resp.data.unwrap().rapid_upload, Some(true));
    }

    #[test]
    fn test_personal_upload_resp_file_exists() {
        let json = r#"{
            "success": true,
            "data": {
                "fileId": "123",
                "fileName": "test.txt",
                "exist": true
            }
        }"#;
        let resp: PersonalUploadResp = serde_json::from_str(json).unwrap();
        assert!(resp.base.success);
        assert_eq!(resp.data.unwrap().exist, Some(true));
    }

    #[test]
    fn test_personal_upload_resp_with_parts() {
        let json = r#"{
            "success": true,
            "data": {
                "fileId": "123",
                "fileName": "test.txt",
                "uploadId": "upload_123",
                "partInfos": [
                    {"partNumber": 1, "uploadUrl": "http://upload1.example.com"},
                    {"partNumber": 2, "uploadUrl": "http://upload2.example.com"}
                ]
            }
        }"#;
        let resp: PersonalUploadResp = serde_json::from_str(json).unwrap();
        let data = resp.data.unwrap();
        assert_eq!(data.part_infos.unwrap().len(), 2);
    }

    #[test]
    fn test_route_policy_full() {
        let json = r#"{
            "siteID": "site1",
            "siteCode": "code1",
            "modName": "personal",
            "httpUrl": "http://example.com",
            "httpsUrl": "https://example.com"
        }"#;
        let policy: RoutePolicy = serde_json::from_str(json).unwrap();
        assert_eq!(policy.site_id, Some("site1".to_string()));
        assert_eq!(policy.mod_name, Some("personal".to_string()));
        assert_eq!(policy.https_url, Some("https://example.com".to_string()));
    }

    #[test]
    fn test_route_policy_empty() {
        let json = r#"{}"#;
        let policy: RoutePolicy = serde_json::from_str(json).unwrap();
        assert_eq!(policy.site_id, None);
        assert_eq!(policy.mod_name, None);
    }
}
