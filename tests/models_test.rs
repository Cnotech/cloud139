use cloud139::models::*;

#[test]
fn test_base_resp_deserialize() {
    let json = r#"{"success": true, "code": "0", "message": "ok"}"#;
    let resp: BaseResp = serde_json::from_str(json).unwrap();
    assert!(resp.success);
    assert_eq!(resp.code, Some("0".to_string()));
    assert_eq!(resp.message, Some("ok".to_string()));
}

#[test]
fn test_base_resp_default() {
    let json = r#"{"success": false}"#;
    let resp: BaseResp = serde_json::from_str(json).unwrap();
    assert!(!resp.success);
    assert_eq!(resp.code, None);
    assert_eq!(resp.message, None);
}

#[test]
fn test_api_result_deserialize() {
    let json = r#"{"resultCode": "0", "resultDesc": "success"}"#;
    let result: ApiResult = serde_json::from_str(json).unwrap();
    assert_eq!(result.result_code, "0");
    assert_eq!(result.result_desc, Some("success".to_string()));
}

#[test]
fn test_common_account_info() {
    let info = CommonAccountInfo {
        account: "13800138000".to_string(),
        account_type: 1,
    };
    let json = serde_json::to_string(&info).unwrap();
    assert!(json.contains("\"account\":\"13800138000\""));
    assert!(json.contains("\"accountType\":1"));
}

#[test]
fn test_create_batch_opr_task_resp_deserialize() {
    let json = r#"{
        "result": {"resultCode": "0", "resultDesc": "ok"},
        "taskID": "task123"
    }"#;
    let resp: CreateBatchOprTaskResp = serde_json::from_str(json).unwrap();
    assert_eq!(resp.result.result_code, "0");
    assert_eq!(resp.task_id, "task123");
}

#[test]
fn test_part_info_deserialize() {
    let json = r#"{
        "partNumber": 1,
        "partSize": 1048576,
        "parallelHashCtx": {"partOffset": 0}
    }"#;
    let part: PartInfo = serde_json::from_str(json).unwrap();
    assert_eq!(part.part_number, 1);
    assert_eq!(part.part_size, 1048576);
    assert_eq!(part.parallel_hash_ctx.part_offset, 0);
}

#[test]
fn test_query_route_policy_resp_deserialize() {
    let json = r#"{
        "success": true,
        "code": "0",
        "message": "ok",
        "data": {
            "routePolicyList": [
                {"modName": "personal", "httpsUrl": "https://example.com"}
            ]
        }
    }"#;
    let resp: QueryRoutePolicyResp = serde_json::from_str(json).unwrap();
    assert!(resp.success);
    assert_eq!(resp.data.route_policy_list.len(), 1);
    assert_eq!(
        resp.data.route_policy_list[0].mod_name,
        Some("personal".to_string())
    );
}

#[test]
fn test_route_policy_defaults() {
    let json = r#"{}"#;
    let policy: RoutePolicy = serde_json::from_str(json).unwrap();
    assert_eq!(policy.site_id, None);
    assert_eq!(policy.mod_name, None);
    assert_eq!(policy.https_url, None);
}

#[test]
fn test_personal_list_resp_deserialize() {
    let json = r#"{
        "success": true,
        "data": {
            "items": [
                {"fileId": "123", "name": "test.txt", "size": 1024}
            ],
            "nextPageCursor": "abc"
        }
    }"#;
    let resp: PersonalListResp = serde_json::from_str(json).unwrap();
    assert!(resp.base.success);
    let data = resp.data.unwrap();
    assert_eq!(data.items.len(), 1);
    assert_eq!(data.items[0].name, Some("test.txt".to_string()));
    assert_eq!(data.next_page_cursor, Some("abc".to_string()));
}

#[test]
fn test_personal_file_item_defaults() {
    let json = r#"{}"#;
    let item: PersonalFileItem = serde_json::from_str(json).unwrap();
    assert_eq!(item.file_id, None);
    assert_eq!(item.name, None);
    assert_eq!(item.size, None);
}

#[test]
fn test_personal_thumbnail() {
    let json = r#"{"style": "small", "url": "http://example.com/img.jpg"}"#;
    let thumb: PersonalThumbnail = serde_json::from_str(json).unwrap();
    assert_eq!(thumb.style, Some("small".to_string()));
    assert_eq!(thumb.url, Some("http://example.com/img.jpg".to_string()));
}

#[test]
fn test_personal_upload_resp_deserialize() {
    let json = r#"{
        "success": true,
        "data": {
            "fileId": "123",
            "fileName": "test.txt",
            "exist": false,
            "rapidUpload": true
        }
    }"#;
    let resp: PersonalUploadResp = serde_json::from_str(json).unwrap();
    assert!(resp.base.success);
    let data = resp.data.unwrap();
    assert_eq!(data.file_id, Some("123".to_string()));
    assert_eq!(data.exist, Some(false));
    assert_eq!(data.rapid_upload, Some(true));
}

#[test]
fn test_download_url_resp_deserialize() {
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
    assert_eq!(resp.data.url, Some("http://example.com/file".to_string()));
    assert_eq!(
        resp.data.cdn_url,
        Some("http://cdn.example.com/file".to_string())
    );
}

#[test]
fn test_query_content_list_resp_deserialize() {
    let json = r#"{
        "success": true,
        "data": {
            "result": {"resultCode": "0"},
            "path": "/test",
            "cloudContentList": [],
            "cloudCatalogList": [],
            "totalCount": 0
        }
    }"#;
    let resp: QueryContentListResp = serde_json::from_str(json).unwrap();
    assert!(resp.base.success);
    assert_eq!(resp.data.path, "/test");
}

#[test]
fn test_cloud_content_deserialize() {
    let json = r#"{
        "contentID": "123",
        "contentName": "test.txt",
        "contentSize": 1024,
        "createTime": "2024-01-01",
        "lastUpdateTime": "2024-01-02",
        "thumbnailURL": "http://example.com/thumb.jpg"
    }"#;
    let content: CloudContent = serde_json::from_str(json).unwrap();
    assert_eq!(content.content_id, "123");
    assert_eq!(content.content_name, "test.txt");
    assert_eq!(content.content_size, 1024);
}

#[test]
fn test_cloud_catalog_deserialize() {
    let json = r#"{
        "catalogID": "123",
        "catalogName": "folder",
        "createTime": "2024-01-01",
        "lastUpdateTime": "2024-01-02"
    }"#;
    let catalog: CloudCatalog = serde_json::from_str(json).unwrap();
    assert_eq!(catalog.catalog_id, "123");
    assert_eq!(catalog.catalog_name, "folder");
}

#[test]
fn test_query_group_content_list_resp_deserialize() {
    let json = r#"{
        "success": true,
        "data": {
            "result": {"resultCode": "0"},
            "getGroupContentResult": {
                "parentCatalogID": "parent123",
                "catalogList": [],
                "contentList": [],
                "nodeCount": 0,
                "ctlgCnt": 0,
                "contCnt": 0
            }
        }
    }"#;
    let resp: QueryGroupContentListResp = serde_json::from_str(json).unwrap();
    assert!(resp.base.success);
    assert_eq!(
        resp.data.get_group_content_result.parent_catalog_id,
        "parent123"
    );
}

#[test]
fn test_group_catalog_deserialize() {
    let json = r#"{
        "catalogID": "123",
        "catalogName": "folder",
        "createTime": "2024-01-01",
        "updateTime": "2024-01-02",
        "path": "/folder"
    }"#;
    let catalog: GroupCatalog = serde_json::from_str(json).unwrap();
    assert_eq!(catalog.catalog_id, "123");
    assert_eq!(catalog.path, "/folder");
}

#[test]
fn test_group_content_deserialize() {
    let json = r#"{
        "contentID": "123",
        "contentName": "test.txt",
        "contentSize": 1024,
        "createTime": "2024-01-01",
        "updateTime": "2024-01-02"
    }"#;
    let content: GroupContent = serde_json::from_str(json).unwrap();
    assert_eq!(content.content_id, "123");
    assert_eq!(content.content_name, "test.txt");
    assert_eq!(content.content_size, 1024);
}

#[test]
fn test_create_folder_resp_deserialize() {
    let json = r#"{
        "success": true,
        "data": {
            "fileId": "123",
            "fileName": "newfolder"
        }
    }"#;
    let resp: CreateFolderResp = serde_json::from_str(json).unwrap();
    assert!(resp.base.success);
    assert_eq!(resp.data.file_id, "123");
}

#[test]
fn test_batch_responses_deserialize() {
    let json = r#"{"success": true}"#;

    let _: BatchMoveResp = serde_json::from_str(json).unwrap();
    let _: BatchCopyResp = serde_json::from_str(json).unwrap();
    let _: BatchTrashResp = serde_json::from_str(json).unwrap();
    let _: BatchRenameResp = serde_json::from_str(json).unwrap();
    let _: BatchDeleteResp = serde_json::from_str(json).unwrap();
}

#[test]
fn test_list_request_serialize() {
    let req = ListRequest {
        parent_file_id: "parent123".to_string(),
        page_num: 1,
        page_size: 100,
        order_by: Some("name".to_string()),
        descending: Some(true),
    };
    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains("\"parentFileId\":\"parent123\""));
}

#[test]
fn test_upload_request_serialize() {
    let req = UploadRequest {
        content_hash: "abc123".to_string(),
        content_hash_algorithm: "SHA1".to_string(),
        size: 1024,
        parent_file_id: "parent123".to_string(),
        name: "test.txt".to_string(),
        file_rename_mode: Some("overwrite".to_string()),
        file_type: Some("txt".to_string()),
        content_type: Some("text/plain".to_string()),
        common_account_info: Some(CommonAccountInfo {
            account: "13800138000".to_string(),
            account_type: 1,
        }),
    };
    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains("\"contentHash\":\"abc123\""));
}

#[test]
fn test_family_list_request_serialize() {
    let req = FamilyListRequest {
        catalog_id: "catalog123".to_string(),
        content_sort_type: 0,
        sort_direction: 1,
        page_info: PageInfo {
            page_num: 1,
            page_size: 100,
        },
    };
    let json = serde_json::to_string(&req).unwrap();
    assert!(
        json.contains("\"catalogID\":\"catalog123\""),
        "JSON: {}",
        json
    );
}

#[test]
fn test_group_list_request_serialize() {
    let req = GroupListRequest {
        group_id: "group123".to_string(),
        catalog_id: "catalog123".to_string(),
        content_sort_type: 0,
        sort_direction: 1,
        start_number: 0,
        end_number: 100,
        path: "/".to_string(),
    };
    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains("\"groupID\":\"group123\""), "JSON: {}", json);
    assert!(
        json.contains("\"catalogID\":\"catalog123\""),
        "JSON: {}",
        json
    );
}
#[test]
fn test_query_file_resp_deserialize() {
    let json = r#"{
        "success": true,
        "data": {
            "fileId": "123",
            "name": "test.txt",
            "type": "file"
        }
    }"#;
    let resp: QueryFileResp = serde_json::from_str(json).unwrap();
    assert!(resp.base.success);
    assert_eq!(resp.data.file_id, "123");
    assert_eq!(resp.data.name, "test.txt");
}

#[test]
fn test_refresh_token_resp_deserialize() {
    let xml = r#"<root><return>0</return><token>token123</token><accessToken>access123</accessToken></root>"#;
    let resp: RefreshTokenResp = serde_xml_rs::from_str(xml).unwrap();
    assert_eq!(resp.return_code, Some("0".to_string()));
    assert_eq!(resp.token, Some("token123".to_string()));
    assert_eq!(resp.access_token, Some("access123".to_string()));
}
