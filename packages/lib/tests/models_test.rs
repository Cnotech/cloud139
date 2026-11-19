use cloud139_core::models::*;

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
