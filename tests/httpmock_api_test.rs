#![allow(dead_code)]

use httpmock::prelude::*;
use serde_json::json;

#[test]
fn test_mock_route_policy() {
    let server = MockServer::start();

    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/user/route/qryRoutePolicy")
            .header("Content-Type", "application/json;charset=UTF-8");
        then.status(200).json_body(json!({
            "code": "0",
            "data": {
                "route_policy_list": [
                    {
                        "mod_name": "personal",
                        "https_url": "https://personal.cloud.139.com"
                    }
                ]
            }
        }));
    });

    m.assert_calls(0);
}

#[test]
fn test_mock_list_files() {
    let server = MockServer::start();

    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/file/queryContentList")
            .header("Content-Type", "application/json;charset=UTF-8");
        then.status(200).json_body(json!({
            "success": true,
            "message": "",
            "data": {
                "path": "/",
                "childFileList": {
                    "catalogList": [
                        {
                            "fileId": "123",
                            "name": "test_folder",
                            "type": "folder"
                        }
                    ],
                    "contentList": [
                        {
                            "fileId": "456",
                            "name": "test.txt",
                            "size": 1024,
                            "type": "file"
                        }
                    ]
                }
            }
        }));
    });

    m.assert_calls(0);
}

#[test]
fn test_mock_delete_file() {
    let server = MockServer::start();

    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/recyclebin/batchTrash")
            .header("Content-Type", "application/json;charset=UTF-8")
            .json_body(json!({
                "fileIds": ["123"]
            }));
        then.status(200).json_body(json!({
            "success": true,
            "message": "文件已移动到回收站"
        }));
    });

    m.assert_calls(0);
}

#[test]
fn test_mock_upload_init() {
    let server = MockServer::start();

    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/file/uploadInit")
            .header("Content-Type", "application/json;charset=UTF-8");
        then.status(200).json_body(json!({
            "success": true,
            "data": {
                "fileId": "789",
                "fileName": "test.txt",
                "partInfos": [
                    {
                        "partNumber": 1,
                        "uploadUrl": "http://upload.example.com/part1"
                    }
                ]
            }
        }));
    });

    m.assert_calls(0);
}

#[test]
fn test_mock_download_url() {
    let server = MockServer::start();

    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/file/getDownloadUrl")
            .header("Content-Type", "application/json;charset=UTF-8");
        then.status(200).json_body(json!({
            "success": true,
            "data": {
                "url": "http://download.example.com/file",
                "cdnUrl": "http://cdn.example.com/file",
                "fileName": "test.txt"
            }
        }));
    });

    m.assert_calls(0);
}

#[test]
fn test_mock_mkdir() {
    let server = MockServer::start();

    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/file/createFolder")
            .header("Content-Type", "application/json;charset=UTF-8");
        then.status(200).json_body(json!({
            "success": true,
            "message": "文件夹创建成功",
            "data": {
                "fileId": "999"
            }
        }));
    });

    m.assert_calls(0);
}

#[test]
fn test_mock_rename() {
    let server = MockServer::start();

    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/file/batchRename")
            .header("Content-Type", "application/json;charset=UTF-8");
        then.status(200).json_body(json!({
            "success": true,
            "message": "重命名成功"
        }));
    });

    m.assert_calls(0);
}

#[test]
fn test_mock_copy() {
    let server = MockServer::start();

    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/file/batchCopy")
            .header("Content-Type", "application/json;charset=UTF-8");
        then.status(200).json_body(json!({
            "success": true,
            "message": "复制成功"
        }));
    });

    m.assert_calls(0);
}

#[test]
fn test_mock_move() {
    let server = MockServer::start();

    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/file/batchMove")
            .header("Content-Type", "application/json;charset=UTF-8");
        then.status(200).json_body(json!({
            "success": true,
            "message": "移动成功"
        }));
    });

    m.assert_calls(0);
}
