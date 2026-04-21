use crate::client::endpoints::{family, group};
use crate::client::{Client, ClientError, StorageType};
use crate::{debug, error, success, warn};
use clap::Parser;

#[derive(Parser, Debug)]
pub struct RenameArgs {
    #[arg(help = "源文件路径")]
    pub source: String,

    #[arg(help = "新名称")]
    pub target: String,

    #[arg(short, long, help = "强制继续，如果云端存在同名文件则自动重命名")]
    pub force: bool,
}

pub fn validate_rename_path(source: &str) -> Result<(), String> {
    if source == "/" || source.is_empty() {
        return Err("不能重命名根目录".to_string());
    }
    Ok(())
}

pub async fn execute(args: RenameArgs) -> anyhow::Result<()> {
    let config = crate::commands::dispatch::load_config()?;
    let storage_type = config.storage_type();

    match storage_type {
        StorageType::PersonalNew => {
            rename_personal(&config, &args.source, &args.target, args.force).await?;
        }
        StorageType::Family => {
            rename_family(&config, &args.source, &args.target).await?;
        }
        StorageType::Group => {
            rename_group(&config, &args.source, &args.target).await?;
        }
    }

    Ok(())
}

async fn rename_personal(
    config: &crate::config::Config,
    source: &str,
    new_name: &str,
    force: bool,
) -> Result<(), ClientError> {
    if source == "/" || source.is_empty() {
        error!("不能重命名根目录");
        return Err(ClientError::CannotOperateOnRoot);
    }

    let current_name = std::path::Path::new(source)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    if current_name == new_name {
        warn!("新名称「{}」与当前名称一致，无需执行重命名", new_name);
        return Ok(());
    }

    let file_id = crate::client::api::get_file_id_by_path(config, source).await?;
    if file_id.is_empty() {
        error!("无效的文件路径");
        return Err(ClientError::InvalidFilePath);
    }
    debug!("rename_personal: file_id={}", file_id);

    // 获取源文件所在父目录，用于检测同名
    let source_path = std::path::Path::new(source);
    let parent_path = source_path
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "/".to_string());
    let parent_id = if parent_path == "/" || parent_path.is_empty() {
        "/".to_string()
    } else {
        crate::client::api::get_file_id_by_path(config, &parent_path).await?
    };

    if !force {
        let exists = crate::client::api::check_file_exists(config, &parent_id, new_name).await?;
        if exists {
            warn!("云端已存在「{}」，如果继续则云端会自动进行重命名", new_name);
            error!("请使用 --force 参数确认继续");
            return Err(ClientError::ForceRequired);
        }
    }

    let mut config = config.clone();
    let host = crate::client::api::get_personal_cloud_host(&mut config).await?;
    let url = format!("{}/file/update", host);

    let body = serde_json::json!({
        "fileId": file_id,
        "name": new_name,
        "description": ""
    });

    let resp: crate::models::PersonalUploadResp =
        crate::client::api::personal_api_request(&config, &url, body, StorageType::PersonalNew)
            .await?;

    if resp.base.success {
        success!("重命名成功: {}", new_name);
    } else {
        let msg = resp.base.message.as_deref().unwrap_or("未知错误");
        error!("重命名失败: {}", msg);
        return Err(ClientError::Api(msg.to_string()));
    }

    Ok(())
}

async fn rename_family(
    config: &crate::config::Config,
    source: &str,
    new_name: &str,
) -> Result<(), ClientError> {
    let current_name = std::path::Path::new(source)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    if current_name == new_name {
        warn!("新名称「{}」与当前名称一致，无需执行重命名", new_name);
        return Ok(());
    }

    let client = Client::new(config.clone());

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
        "pageSize": 100
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
        error!("文件不存在");
        return Err(ClientError::FileNotFound);
    }

    // 家庭云不支持重命名文件夹
    if is_dir {
        error!("家庭云不支持重命名文件夹");
        return Err(ClientError::UnsupportedFamilyRenameFolder);
    }

    debug!("rename_family: found_id={}, is_dir={}", found_id, is_dir);

    let url = family::orchestration::MODIFY_CONTENT_INFO;

    let body = serde_json::json!({
        "contentID": found_id,
        "contentName": new_name,
        "commonAccountInfo": {
            "account": config.account,
            "accountType": 1
        },
        "path": found_path
    });

    let resp: serde_json::Value = client.api_request_post(url, body).await?;

    if resp
        .get("result")
        .and_then(|r| r.get("resultCode"))
        .and_then(|c| c.as_str())
        == Some("0")
    {
        success!("重命名成功: {}", new_name);
    } else {
        error!("重命名失败: {:?}", resp);
        return Err(ClientError::Api(format!("{:?}", resp)));
    }

    Ok(())
}

async fn rename_group(
    config: &crate::config::Config,
    source: &str,
    new_name: &str,
) -> Result<(), ClientError> {
    let current_name = std::path::Path::new(source)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    if current_name == new_name {
        warn!("新名称「{}」与当前名称一致，无需执行重命名", new_name);
        return Ok(());
    }

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
        error!("文件不存在");
        return Err(ClientError::FileNotFound);
    }

    debug!("rename_group: found_id={}, is_dir={}", found_id, is_dir);

    if is_dir {
        let url = group::orchestration::MODIFY_GROUP_CATALOG;

        let body = serde_json::json!({
            "groupID": config.cloud_id,
            "modifyCatalogID": found_id,
            "modifyCatalogName": new_name,
            "path": found_path,
            "commonAccountInfo": {
                "account": config.account,
                "accountType": 1
            }
        });

        let resp: serde_json::Value = client.api_request_post(url, body).await?;

        if resp
            .get("result")
            .and_then(|r| r.get("resultCode"))
            .and_then(|c| c.as_str())
            == Some("0")
        {
            success!("重命名成功: {}", new_name);
        } else {
            error!("重命名失败: {:?}", resp);
            return Err(ClientError::Api(format!("{:?}", resp)));
        }
    } else {
        let url = group::orchestration::MODIFY_GROUP_CONTENT;

        let body = serde_json::json!({
            "groupID": config.cloud_id,
            "contentID": found_id,
            "contentName": new_name,
            "path": found_path,
            "commonAccountInfo": {
                "account": config.account,
                "accountType": 1
            }
        });

        let resp: serde_json::Value = client.api_request_post(url, body).await?;

        if resp
            .get("result")
            .and_then(|r| r.get("resultCode"))
            .and_then(|c| c.as_str())
            == Some("0")
        {
            success!("重命名成功: {}", new_name);
        } else {
            error!("重命名失败: {:?}", resp);
            return Err(ClientError::Api(format!("{:?}", resp)));
        }
    }

    Ok(())
}
