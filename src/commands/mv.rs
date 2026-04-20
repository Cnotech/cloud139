use crate::client::endpoints::{family, group};
use crate::client::{Client, ClientError, StorageType};
use crate::models::BatchMoveResp;
use crate::{debug, error, success, warn};
use clap::Parser;

#[derive(Parser, Debug)]
pub struct MvArgs {
    #[arg(required = true, help = "源文件路径（支持多个，用空格分隔）")]
    pub source: Vec<String>,

    #[arg(help = "目标路径")]
    pub target: String,

    #[arg(short, long, help = "强制继续，如果云端存在同名文件则自动重命名")]
    pub force: bool,
}

pub async fn execute(args: MvArgs) -> anyhow::Result<()> {
    if args.source.is_empty() {
        error!("错误: 请指定至少一个源文件");
        return Err(ClientError::NoSourceFiles.into());
    }

    if args.source.iter().any(|s| s == "/") {
        error!("错误: 不能移动根目录");
        return Err(ClientError::CannotOperateOnRoot.into());
    }

    let config = crate::commands::dispatch::load_config()?;
    let storage_type = config.storage_type();

    match storage_type {
        StorageType::PersonalNew => {
            mv_personal(&config, &args.source, &args.target, args.force).await?;
        }
        StorageType::Family => {
            mv_family(&config, &args.source, &args.target).await?;
        }
        StorageType::Group => {
            mv_group(&config, &args.source, &args.target).await?;
        }
    }

    Ok(())
}

async fn mv_personal(
    config: &crate::config::Config,
    sources: &[String],
    target: &str,
    force: bool,
) -> Result<(), ClientError> {
    let target_normalized = if target == "/" || target.is_empty() {
        "/".to_string()
    } else {
        target.to_string()
    };

    let mut source_ids: Vec<String> = Vec::new();
    let mut file_names: Vec<String> = Vec::new();

    for source in sources {
        let source_path = std::path::Path::new(source);
        let source_parent = source_path
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        let source_parent_normalized = if source_parent.is_empty() {
            "/".to_string()
        } else {
            source_parent
        };

        if source_parent_normalized == target_normalized {
            warn!("源目录和目标目录相同，跳过: {}", source);
            continue;
        }

        let source_id = crate::client::api::get_file_id_by_path(config, source).await?;
        if source_id.is_empty() {
            warn!("无效的源文件路径: {}", source);
            continue;
        }
        debug!("mv_personal: source={}, id={}", source, source_id);

        let file_name = source_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        source_ids.push(source_id);
        file_names.push(file_name);
    }

    if source_ids.is_empty() {
        error!("错误: 没有有效的源文件需要移动");
        return Err(ClientError::NoSourceFiles);
    }

    let target_id = if target == "/" || target.is_empty() {
        "/".to_string()
    } else {
        crate::client::api::get_file_id_by_path(config, target).await?
    };
    debug!("mv_personal: target_id={}", target_id);

    if !force {
        for file_name in &file_names {
            let exists =
                crate::client::api::check_file_exists(config, &target_id, file_name).await?;
            if exists {
                warn!(
                    "云端已存在「{}」，如果继续则云端会自动进行重命名",
                    file_name
                );
                error!("请使用 --force 参数确认继续");
                return Err(ClientError::ForceRequired);
            }
        }
    }

    let mut config = config.clone();
    let host = crate::client::api::get_personal_cloud_host(&mut config).await?;
    let url = format!("{}/file/batchMove", host);

    let body = serde_json::json!({
        "fileIds": source_ids,
        "toParentFileId": target_id
    });

    let resp: BatchMoveResp =
        crate::client::api::personal_api_request(&config, &url, body, StorageType::PersonalNew)
            .await?;

    if resp.base.success {
        success!("移动成功");
    } else {
        let msg = resp.base.message.as_deref().unwrap_or("未知错误");
        error!("移动失败: {}", msg);
        return Err(ClientError::Api(msg.to_string()));
    }

    Ok(())
}

async fn mv_family(
    config: &crate::config::Config,
    sources: &[String],
    target: &str,
) -> Result<(), ClientError> {
    if sources.len() > 1 {
        error!("家庭云暂不支持批量移动");
        return Err(ClientError::UnsupportedFamilyBatchMove);
    }

    let client = Client::new(config.clone());

    let source = &sources[0];
    let source = source.trim_start_matches('/');

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

    let list_resp: serde_json::Value = client.api_request_post(url, list_body).await?;

    let mut is_dir = false;
    let mut found_id = String::new();
    let mut found_path = String::new();

    if let Some(catalog_list) = list_resp
        .pointer("/data/cloudCatalogList")
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
                break;
            }
        }
    }

    if !is_dir
        && found_id.is_empty()
        && let Some(content_list) = list_resp
            .pointer("/data/cloudContentList")
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
                    .pointer("/data/path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                break;
            }
        }
    }

    if found_id.is_empty() {
        error!("错误: 文件不存在");
        return Err(ClientError::FileNotFound);
    }

    debug!("mv_family: found_id={}, is_dir={}", found_id, is_dir);

    let target = target.trim_start_matches('/');
    let target_catalog_id = if target.is_empty() {
        "0".to_string()
    } else {
        target.to_string()
    };

    let full_source_path = if found_path.is_empty() {
        format!("root:/{}", found_id)
    } else {
        format!("{}/{}", found_path.trim_end_matches('/'), found_id)
    };

    let target_path = if target.is_empty() {
        config
            .root_folder_id
            .clone()
            .unwrap_or_else(|| "0".to_string())
    } else {
        format!("{}/{}", target, "")
    };

    let body = serde_json::json!({
        "catalogList": if is_dir { vec![full_source_path.clone()] } else { vec![] },
        "accountInfo": {
            "accountName": config.account,
            "accountType": "1"
        },
        "contentList": if !is_dir { vec![full_source_path.clone()] } else { vec![] },
        "destCatalogID": target_catalog_id,
        "destGroupID": config.cloud_id,
        "destPath": target_path,
        "destType": 0,
        "srcGroupID": config.cloud_id,
        "srcType": 0,
        "taskType": 3
    });

    let resp: serde_json::Value = client
        .isbo_post("/isbo/openApi/createBatchOprTask", body)
        .await?;

    if resp
        .get("result")
        .and_then(|r| r.get("resultCode"))
        .and_then(|c| c.as_str())
        == Some("0")
    {
        success!("移动成功");
    } else {
        error!("移动失败: {:?}", resp);
        return Err(ClientError::Api(format!("{:?}", resp)));
    }

    Ok(())
}

async fn mv_group(
    config: &crate::config::Config,
    sources: &[String],
    target: &str,
) -> Result<(), ClientError> {
    if sources.len() > 1 {
        error!("群组云暂不支持批量移动");
        return Err(ClientError::UnsupportedGroupBatchMove);
    }

    let source = &sources[0];
    let source = source.trim_start_matches('/');

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

    let url = group::orchestration::QUERY_GROUP_CONTENT_LIST;

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
        error!("错误: 文件不存在");
        return Err(ClientError::FileNotFound);
    }

    debug!("mv_group: found_id={}, is_dir={}", found_id, is_dir);

    let target = target.trim_start_matches('/');
    let dest_path = if target.is_empty() {
        "root:".to_string()
    } else {
        format!("root:/{}", target)
    };

    let move_url = group::orchestration::CREATE_BATCH_OPR_TASK;

    let full_source_path = if found_path.is_empty() {
        format!("root:/{}", found_id)
    } else {
        format!("{}/{}", found_path.trim_end_matches('/'), found_id)
    };

    let body = if is_dir {
        serde_json::json!({
            "taskType": 3,
            "srcType": 2,
            "srcGroupID": config.cloud_id,
            "destType": 2,
            "destGroupID": config.cloud_id,
            "destPath": dest_path,
            "contentList": [],
            "catalogList": [full_source_path],
            "commonAccountInfo": {
                "account": config.account,
                "accountType": 1
            }
        })
    } else {
        serde_json::json!({
            "taskType": 3,
            "srcType": 2,
            "srcGroupID": config.cloud_id,
            "destType": 2,
            "destGroupID": config.cloud_id,
            "destPath": dest_path,
            "contentList": [full_source_path],
            "catalogList": [],
            "commonAccountInfo": {
                "account": config.account,
                "accountType": 1
            }
        })
    };

    let resp: serde_json::Value = client.api_request_post(move_url, body).await?;

    if resp
        .get("result")
        .and_then(|r| r.get("resultCode"))
        .and_then(|c| c.as_str())
        == Some("0")
    {
        success!("移动成功");
    } else {
        error!("移动失败: {:?}", resp);
        return Err(ClientError::Api(format!("{:?}", resp)));
    }

    Ok(())
}
