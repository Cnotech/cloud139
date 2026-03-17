use crate::client::{Client, ClientError, StorageType};
use crate::models::BatchTrashResp;
use crate::{error, info, success, warn};
use clap::Parser;

#[derive(Parser, Debug)]
pub struct DeleteArgs {
    #[arg(help = "远程文件路径")]
    pub path: String,

    #[arg(short, long, help = "确认删除")]
    pub yes: bool,

    #[arg(short, long, help = "永久删除（不移动到回收站）")]
    pub permanent: bool,
}

pub async fn execute(args: DeleteArgs) -> Result<(), ClientError> {
    if !args.yes {
        if args.permanent {
            warn!("此操作将永久删除文件，无法恢复！");
        } else {
            warn!("此操作会将文件移动到回收站");
        }
        info!("使用 --yes 参数确认删除");
        return Err(ClientError::ConfirmationRequired);
    }

    let config = crate::config::Config::load().map_err(ClientError::Config)?;
    let storage_type = config.storage_type();

    match storage_type {
        StorageType::PersonalNew => {
            delete_personal(&config, &args.path, args.permanent).await?;
        }
        StorageType::Family => {
            delete_family(&config, &args.path, args.permanent).await?;
        }
        StorageType::Group => {
            delete_group(&config, &args.path, args.permanent).await?;
        }
    }

    Ok(())
}

async fn delete_personal(
    config: &crate::config::Config,
    path: &str,
    _permanent: bool,
) -> Result<(), ClientError> {
    if path == "/" || path.is_empty() {
        error!("不能删除根目录");
        return Err(ClientError::CannotOperateOnRoot);
    }

    let file_id = crate::client::api::get_file_id_by_path(config, path).await?;
    if file_id.is_empty() {
        error!("无效的文件路径");
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

    if resp.base.success {
        success!("文件已移动到回收站");
    } else {
        let msg = resp.base.message.as_deref().unwrap_or("未知错误");
        error!("删除失败: {}", msg);
        return Err(ClientError::Api(msg.to_string()));
    }

    Ok(())
}

async fn delete_family(
    config: &crate::config::Config,
    path: &str,
    permanent: bool,
) -> Result<(), ClientError> {
    let (catalog_list, content_list, _) = get_family_file_info(config, path).await?;

    let task_type = if permanent { 3 } else { 2 };
    let url = "https://yun.139.com/orchestration/familyCloud-rebuild/batchOprTask/v1.0/createBatchOprTask";

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
        == Some("0")
    {
        if permanent {
            success!("文件已永久删除");
        } else {
            success!("文件已移动到回收站");
        }
    } else {
        error!("删除失败: {:?}", resp);
        return Err(ClientError::Api(format!("{:?}", resp)));
    }

    Ok(())
}

async fn get_family_file_info(
    config: &crate::config::Config,
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

    let url = "https://yun.139.com/orchestration/familyCloud-rebuild/content/v1.2/queryContentList";

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

    if !is_dir {
        if let Some(content_list) = list_resp
            .pointer("/data/cloudContentList")
            .and_then(|v| v.as_array())
        {
            for content in content_list {
                if content.get("contentName").and_then(|v| v.as_str()) == Some(&file_name) {
                    break;
                }
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

async fn delete_group(
    config: &crate::config::Config,
    path: &str,
    permanent: bool,
) -> Result<(), ClientError> {
    if path == "/" || path.is_empty() {
        error!("不能删除根目录");
        return Err(ClientError::CannotOperateOnRoot);
    }

    let url = "https://yun.139.com/orchestration/group-rebuild/content/v1.0/queryGroupContentList";

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

    if !is_dir && found_id.is_empty() {
        if let Some(content_list) = list_resp
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
    }

    if found_id.is_empty() {
        error!("文件不存在");
        return Err(ClientError::FileNotFound);
    }

    let task_type = if permanent { 3 } else { 2 };
    let delete_url = "https://yun.139.com/orchestration/group-rebuild/task/v1.0/createBatchOprTask";

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
        == Some("0")
    {
        if permanent {
            success!("文件已永久删除");
        } else {
            success!("文件已移动到回收站");
        }
    } else {
        error!("删除失败: {:?}", resp);
        return Err(ClientError::Api(format!("{:?}", resp)));
    }

    Ok(())
}
