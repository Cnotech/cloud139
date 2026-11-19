use cloud139::models::*;

#[test]
fn test_query_file_resp_with_defaults() {
    let json = r#"{"success": true, "data": {"fileId": "123", "name": "test", "type": "file"}}"#;
    let resp: QueryFileResp = serde_json::from_str(json).unwrap();
    assert!(resp.base.success);
}

#[test]
fn test_personal_file_item_with_all_fields() {
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
fn test_personal_upload_data_defaults() {
    let json = r#"{}"#;
    let data: PersonalUploadData = serde_json::from_str(json).unwrap();
    assert_eq!(data.file_id, None);
    assert_eq!(data.file_name, None);
    assert!(data.part_infos.is_none());
    assert_eq!(data.exist, None);
}

#[test]
fn test_personal_part_info_deserialize() {
    let json = r#"{
        "partNumber": 1,
        "uploadUrl": "http://example.com/upload"
    }"#;
    let part: PersonalPartInfo = serde_json::from_str(json).unwrap();
    assert_eq!(part.part_number, 1);
    assert_eq!(part.upload_url, "http://example.com/upload");
}

#[test]
fn test_download_url_data_defaults() {
    let json = r#"{}"#;
    let data: DownloadUrlData = serde_json::from_str(json).unwrap();
    assert_eq!(data.url, None);
    assert_eq!(data.cdn_url, None);
    assert_eq!(data.file_name, None);
}

#[test]
fn test_route_policy_full_deserialize() {
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
fn test_personal_list_data_without_next_cursor() {
    let json = r#"{
        "success": true,
        "data": {
            "items": []
        }
    }"#;
    let resp: PersonalListResp = serde_json::from_str(json).unwrap();
    let data = resp.data.unwrap();
    assert_eq!(data.next_page_cursor, None);
}

#[test]
fn test_batch_responses_with_error() {
    let json = r#"{"success": false, "code": "500", "message": "error"}"#;
    let resp: BatchMoveResp = serde_json::from_str(json).unwrap();
    assert!(!resp.base.success);
    assert_eq!(resp.base.code, Some("500".to_string()));
}

#[test]
fn test_common_account_info_default() {
    let info = CommonAccountInfo {
        account: "".to_string(),
        account_type: 0,
    };
    let json = serde_json::to_string(&info).unwrap();
    assert!(json.contains("\"account\":\"\""));
    assert!(json.contains("\"accountType\":0"));
}

#[test]
fn test_api_result_defaults() {
    let json = r#"{"resultCode": "0"}"#;
    let result: ApiResult = serde_json::from_str(json).unwrap();
    assert_eq!(result.result_desc, None);
}

#[test]
fn test_part_info_defaults() {
    let json = r#"{"partNumber": 1, "partSize": 1024, "parallelHashCtx": {"partOffset": 0}}"#;
    let part: PartInfo = serde_json::from_str(json).unwrap();
    assert_eq!(part.parallel_hash_ctx.part_offset, 0);
}
