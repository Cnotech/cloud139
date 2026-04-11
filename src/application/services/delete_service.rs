use crate::client::endpoints::{family, group};
use crate::client::{Client, ClientError, StorageType};
use crate::config::Config;
use crate::models::BatchTrashResp;
use anyhow::Result;

/// 删除文件服务
pub async fn delete(config: &Config, path: &str, permanent: bool) -> Result<()> {
    let storage_type = config.storage_type();

    match storage_type {
        StorageType::PersonalNew => delete_personal(config, path, permanent).await?,
        StorageType::Family => delete_family(config, path, permanent).await?,
        StorageType::Group => delete_group(config, path, permanent).await?,
    }

    Ok(())
}

/// 删除个人云文件
async fn delete_personal(config: &Config, path: &str, _permanent: bool) -> Result<(), ClientError> {
    if path == "/" || path.is_empty() {
        return Err(ClientError::CannotOperateOnRoot);
    }

    let file_id = crate::client::api::get_file_id_by_path(config, path).await?;
    if file_id.is_empty() {
        return Err(ClientError::InvalidFilePath);
    }

    let mut config = config.clone();
    let host = crate::client::api::get_personal_cloud_host(&mut config).await?;

    let url = format!("{}/recyclebin/batchTrash", host);

    let body = serde_json::json!({
        "fileIds": [file_id]
    });

    let resp: BatchTrashResp =
        crate::client::api::personal_api_request(&config, &url, body, StorageType::PersonalNew)
            .await?;

    if !resp.base.success {
        let msg = resp.base.message.as_deref().unwrap_or("未知错误");
        return Err(ClientError::Api(msg.to_string()));
    }

    Ok(())
}

/// 删除家庭云文件
async fn delete_family(config: &Config, path: &str, permanent: bool) -> Result<(), ClientError> {
    let (catalog_list, content_list, _) = get_family_file_info(config, path).await?;

    let task_type = if permanent { 3 } else { 2 };
    let url = family::orchestration::CREATE_BATCH_OPR_TASK;

    let body = serde_json::json!({
        "catalogList": catalog_list,
        "contentList": content_list,
        "commonAccountInfo": {
            "account": config.account,
            "accountType": 1
        },
        "sourceCloudID": config.cloud_id,
        "sourceCatalogType": 1002,
        "taskType": task_type,
        "path": format!("root:/{}", path.trim_start_matches('/'))
    });

    let client = Client::new(config.clone());
    let resp: serde_json::Value = client.api_request_post(url, body).await?;

    if resp
        .get("result")
        .and_then(|r| r.get("resultCode"))
        .and_then(|c| c.as_str())
        != Some("0")
    {
        return Err(ClientError::Api(format!("{:?}", resp)));
    }

    Ok(())
}

/// 获取家庭云文件信息
async fn get_family_file_info(
    config: &Config,
    path: &str,
) -> Result<(Vec<String>, Vec<String>, bool), ClientError> {
    let source = path.trim_start_matches('/');
    let parent_path = std::path::Path::new(source);
    let parent_dir = parent_path
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let file_name = parent_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    let catalog_id = if parent_dir.is_empty() {
        "0".to_string()
    } else {
        parent_dir.clone()
    };

    let url = family::orchestration::QUERY_CONTENT_LIST;

    let list_body = serde_json::json!({
        "catalogID": catalog_id,
        "sortType": 1,
        "pageNumber": 1,
        "pageSize": 100,
        "cloudID": config.cloud_id,
        "cloudType": 1,
        "commonAccountInfo": {
            "account": config.account,
            "accountType": 1
        }
    });

    let client = Client::new(config.clone());
    let list_resp: serde_json::Value = client.api_request_post(url, list_body).await?;

    let mut is_dir = false;

    if let Some(catalog_list) = list_resp
        .pointer("/data/cloudCatalogList")
        .and_then(|v| v.as_array())
    {
        for cat in catalog_list {
            if cat.get("catalogName").and_then(|v| v.as_str()) == Some(&file_name) {
                is_dir = true;
                break;
            }
        }
    }

    if !is_dir
        && let Some(content_list) = list_resp
            .pointer("/data/cloudContentList")
            .and_then(|v| v.as_array())
    {
        for content in content_list {
            if content.get("contentName").and_then(|v| v.as_str()) == Some(&file_name) {
                break;
            }
        }
    }

    if is_dir {
        Ok((
            vec![format!("root:/{}", path.trim_start_matches('/'))],
            vec![],
            true,
        ))
    } else {
        Ok((
            vec![],
            vec![format!("root:/{}", path.trim_start_matches('/'))],
            false,
        ))
    }
}

/// 删除群组云文件
async fn delete_group(config: &Config, path: &str, permanent: bool) -> Result<(), ClientError> {
    if path == "/" || path.is_empty() {
        return Err(ClientError::CannotOperateOnRoot);
    }

    let url = group::orchestration::QUERY_GROUP_CONTENT_LIST;

    let parent_path = std::path::Path::new(path);
    let parent_dir = parent_path
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let file_name = parent_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    let catalog_id = if parent_dir.is_empty() {
        "0".to_string()
    } else {
        parent_dir.clone()
    };
    let list_body = serde_json::json!({
        "groupID": config.cloud_id,
        "catalogID": catalog_id,
        "contentSortType": 0,
        "sortDirection": 1,
        "startNumber": 1,
        "endNumber": 100,
        "path": if parent_dir.is_empty() { "root:".to_string() } else { format!("root:/{}", parent_dir) }
    });

    let client = Client::new(config.clone());
    let list_resp: serde_json::Value = client.api_request_post(url, list_body).await?;

    let mut is_dir = false;
    let mut found_id = String::new();
    let mut found_path = String::new();

    if let Some(catalog_list) = list_resp
        .pointer("/data/getGroupContentResult/catalogList")
        .and_then(|v| v.as_array())
    {
        for cat in catalog_list {
            if cat.get("catalogName").and_then(|v| v.as_str()) == Some(&file_name) {
                is_dir = true;
                found_id = cat
                    .get("catalogID")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                found_path = cat
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                break;
            }
        }
    }

    if !is_dir
        && found_id.is_empty()
        && let Some(content_list) = list_resp
            .pointer("/data/getGroupContentResult/contentList")
            .and_then(|v| v.as_array())
    {
        for content in content_list {
            if content.get("contentName").and_then(|v| v.as_str()) == Some(&file_name) {
                found_id = content
                    .get("contentID")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                found_path = list_resp
                    .pointer("/data/getGroupContentResult/parentCatalogID")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                break;
            }
        }
    }

    if found_id.is_empty() {
        return Err(ClientError::FileNotFound);
    }

    let task_type = if permanent { 3 } else { 2 };
    let delete_url = group::orchestration::CREATE_BATCH_OPR_TASK;

    let full_path = if is_dir {
        found_path.clone()
    } else {
        format!("{}/{}", found_path.trim_end_matches('/'), found_id)
    };

    let body = if is_dir {
        serde_json::json!({
            "taskType": task_type,
            "srcGroupID": config.cloud_id,
            "contentList": [],
            "catalogList": [full_path],
            "commonAccountInfo": {
                "account": config.account,
                "accountType": 1
            }
        })
    } else {
        serde_json::json!({
            "taskType": task_type,
            "srcGroupID": config.cloud_id,
            "contentList": [full_path],
            "catalogList": [],
            "commonAccountInfo": {
                "account": config.account,
                "accountType": 1
            }
        })
    };

    let resp: serde_json::Value = client.api_request_post(delete_url, body).await?;

    if resp
        .get("result")
        .and_then(|r| r.get("resultCode"))
        .and_then(|c| c.as_str())
        != Some("0")
    {
        return Err(ClientError::Api(format!("{:?}", resp)));
    }

    Ok(())
}
