#![allow(dead_code)]

use cloud139::models::*;

#[test]
fn test_base_resp_deserialize_success() {
    let json = r#"{"success": true, "message": "ok"}"#;
    let resp: BaseResp = serde_json::from_str(json).unwrap();
    assert!(resp.success);
    assert_eq!(resp.message, Some("ok".to_string()));
}

#[test]
fn test_base_resp_deserialize_with_code() {
    let json = r#"{"success": false, "code": "500", "message": "error"}"#;
    let resp: BaseResp = serde_json::from_str(json).unwrap();
    assert!(!resp.success);
    assert_eq!(resp.code, Some("500".to_string()));
}

#[test]
fn test_api_result_deserialize() {
    let json = r#"{"resultCode": "0", "resultDesc": "success"}"#;
    let result: ApiResult = serde_json::from_str(json).unwrap();
    assert_eq!(result.result_code, "0");
    assert_eq!(result.result_desc, Some("success".to_string()));
}

#[test]
fn test_common_account_info_serialize() {
    let info = CommonAccountInfo {
        account: "test@139.com".to_string(),
        account_type: 1,
    };
    let json = serde_json::to_string(&info).unwrap();
    assert!(json.contains("test@139.com"));
    assert!(json.contains("accountType"));
}

#[test]
fn test_route_policy_deserialize_full() {
    let json = r#"{
        "siteID": "123",
        "siteCode": "ABC",
        "modName": "personal",
        "httpUrl": "http://example.com",
        "httpsUrl": "https://example.com"
    }"#;
    let policy: RoutePolicy = serde_json::from_str(json).unwrap();
    assert_eq!(policy.mod_name, Some("personal".to_string()));
    assert_eq!(policy.https_url, Some("https://example.com".to_string()));
}

#[test]
fn test_route_policy_deserialize_partial() {
    let json = r#"{"modName": "personal"}"#;
    let policy: RoutePolicy = serde_json::from_str(json).unwrap();
    assert_eq!(policy.mod_name, Some("personal".to_string()));
    assert!(policy.site_id.is_none());
}

#[test]
fn test_personal_file_item_all_fields() {
    let json = r#"{
        "fileId": "12345",
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
    assert_eq!(item.file_id, Some("12345".to_string()));
    assert_eq!(item.name, Some("test.txt".to_string()));
    assert_eq!(item.size, Some(1024));
    assert_eq!(item.file_type, Some("file".to_string()));
}

#[test]
fn test_personal_thumbnail_deserialize() {
    let json = r#"{"style": "large", "url": "http://example.com/thumb.jpg"}"#;
    let thumb: PersonalThumbnail = serde_json::from_str(json).unwrap();
    assert_eq!(thumb.style, Some("large".to_string()));
    assert_eq!(thumb.url, Some("http://example.com/thumb.jpg".to_string()));
}

#[test]
fn test_personal_upload_data_with_exist() {
    let json = r#"{
        "fileId": "123",
        "fileName": "test.txt",
        "exist": true,
        "rapidUpload": true
    }"#;
    let data: PersonalUploadData = serde_json::from_str(json).unwrap();
    assert_eq!(data.file_id, Some("123".to_string()));
    assert_eq!(data.exist, Some(true));
    assert_eq!(data.rapid_upload, Some(true));
}

#[test]
fn test_cloud_content_deserialize() {
    let json = r#"{
        "contentID": "c123",
        "contentName": "document.pdf",
        "contentSize": 2048,
        "createTime": "2024-01-01",
        "lastUpdateTime": "2024-01-02"
    }"#;
    let content: CloudContent = serde_json::from_str(json).unwrap();
    assert_eq!(content.content_id, "c123");
    assert_eq!(content.content_name, "document.pdf");
    assert_eq!(content.content_size, 2048);
}

#[test]
fn test_cloud_catalog_deserialize() {
    let json = r#"{
        "catalogID": "cat123",
        "catalogName": "folder",
        "createTime": "2024-01-01",
        "lastUpdateTime": "2024-01-02"
    }"#;
    let catalog: CloudCatalog = serde_json::from_str(json).unwrap();
    assert_eq!(catalog.catalog_id, "cat123");
    assert_eq!(catalog.catalog_name, "folder");
}

#[test]
fn test_group_catalog_deserialize() {
    let json = r#"{
        "catalogID": "g123",
        "catalogName": "group_folder",
        "createTime": "2024-01-01",
        "updateTime": "2024-01-02",
        "path": "root:/folder"
    }"#;
    let catalog: GroupCatalog = serde_json::from_str(json).unwrap();
    assert_eq!(catalog.catalog_id, "g123");
    assert_eq!(catalog.path, "root:/folder");
}

#[test]
fn test_group_content_deserialize() {
    let json = r#"{
        "contentID": "gc123",
        "contentName": "group_file.txt",
        "contentSize": 512,
        "createTime": "2024-01-01",
        "updateTime": "2024-01-02",
        "digest": "abc123"
    }"#;
    let content: GroupContent = serde_json::from_str(json).unwrap();
    assert_eq!(content.content_id, "gc123");
    assert_eq!(content.digest, Some("abc123".to_string()));
}

#[test]
fn test_create_folder_data_deserialize() {
    let json = r#"{"fileId": "f123", "fileName": "new_folder"}"#;
    let data: CreateFolderData = serde_json::from_str(json).unwrap();
    assert_eq!(data.file_id, "f123");
    assert_eq!(data.file_name, "new_folder");
}

#[test]
fn test_query_file_data_deserialize() {
    let json = r#"{"fileId": "q123", "name": "query_file.txt", "type": "file"}"#;
    let data: QueryFileData = serde_json::from_str(json).unwrap();
    assert_eq!(data.file_id, "q123");
    assert_eq!(data.name, "query_file.txt");
    assert_eq!(data.file_type, "file");
}

#[test]
fn test_list_request_serialize() {
    let request = ListRequest {
        parent_file_id: "/".to_string(),
        page_num: 1,
        page_size: 100,
        order_by: Some("name".to_string()),
        descending: Some(true),
    };
    let json = serde_json::to_string(&request).unwrap();
    assert!(json.contains("parentFileId"));
    assert!(json.contains("pageNum"));
}

#[test]
fn test_upload_request_serialize() {
    let request = UploadRequest {
        content_hash: "abc123".to_string(),
        content_hash_algorithm: "SHA256".to_string(),
        size: 1024,
        parent_file_id: "/".to_string(),
        name: "test.txt".to_string(),
        file_rename_mode: Some("auto".to_string()),
        file_type: Some("file".to_string()),
        content_type: Some("text/plain".to_string()),
        common_account_info: None,
    };
    let json = serde_json::to_string(&request).unwrap();
    assert!(json.contains("contentHash"));
    assert!(json.contains("test.txt"));
}

#[test]
fn test_page_info_serialize() {
    let info = PageInfo {
        page_num: 1,
        page_size: 50,
    };
    let json = serde_json::to_string(&info).unwrap();
    assert!(json.contains("pageNum"));
    assert!(json.contains("pageSize"));
}

#[test]
fn test_family_create_folder_request_serialize() {
    let request = FamilyCreateFolderRequest {
        catalog_name: "new_folder".to_string(),
        parent_catalog_id: "parent123".to_string(),
    };
    let json = serde_json::to_string(&request).unwrap();
    assert!(json.contains("new_folder"));
    assert!(json.contains("parent123"));
}
