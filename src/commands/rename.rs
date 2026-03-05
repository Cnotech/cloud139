use clap::Parser;
use crate::client::{Client, ClientError, StorageType};

#[derive(Parser, Debug)]
pub struct RenameArgs {
    #[arg(help = "源文件路径")]
    pub source: String,

    #[arg(help = "新名称")]
    pub target: String,
}

pub async fn execute(args: RenameArgs) -> Result<(), ClientError> {
    let config = crate::config::Config::load().map_err(ClientError::Config)?;
    let storage_type = config.storage_type();

    match storage_type {
        StorageType::PersonalNew => {
            rename_personal(&config, &args.source, &args.target).await?;
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

async fn rename_personal(config: &crate::config::Config, source: &str, new_name: &str) -> Result<(), ClientError> {
    if source == "/" || source.is_empty() {
        println!("错误: 不能重命名根目录");
        return Ok(());
    }

    let file_id = crate::client::api::get_file_id_by_path(config, source).await?;
    if file_id.is_empty() {
        println!("错误: 无效的文件路径");
        return Ok(());
    }

    let mut config = config.clone();
    let host = crate::client::api::get_personal_cloud_host(&mut config).await?;
    let url = format!("{}/file/update", host);

    let body = serde_json::json!({
        "fileId": file_id,
        "name": new_name,
        "description": ""
    });

    let resp: crate::models::PersonalUploadResp = crate::client::api::personal_api_request(&config, &url, body, StorageType::PersonalNew).await?;

    if resp.base.success {
        println!("重命名成功: {}", new_name);
    } else {
        println!("重命名失败: {}", resp.base.message.as_deref().unwrap_or("未知错误"));
    }

    Ok(())
}

async fn rename_family(config: &crate::config::Config, source: &str, new_name: &str) -> Result<(), ClientError> {
    let client = Client::new(config.clone());
    
    let source = source.trim_start_matches('/');
    let parent_path = std::path::Path::new(source);
    let parent_dir = parent_path.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
    let file_name = parent_path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    
    let catalog_id = if parent_dir.is_empty() { "0".to_string() } else { parent_dir.clone() };
    
    let url = "https://yun.139.com/orchestration/familyCloud-rebuild/content/v1.2/queryContentList";
    
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
    
    if let Some(catalog_list) = list_resp.pointer("/data/cloudCatalogList").and_then(|v| v.as_array()) {
        for cat in catalog_list {
            if cat.get("catalogName").and_then(|v| v.as_str()) == Some(&file_name) {
                is_dir = true;
                found_id = cat.get("catalogID").and_then(|v| v.as_str()).unwrap_or("").to_string();
                break;
            }
        }
    }
    
    if !is_dir && found_id.is_empty() {
        if let Some(content_list) = list_resp.pointer("/data/cloudContentList").and_then(|v| v.as_array()) {
            for content in content_list {
                if content.get("contentName").and_then(|v| v.as_str()) == Some(&file_name) {
                    found_id = content.get("contentID").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    found_path = list_resp.pointer("/data/path").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    break;
                }
            }
        }
    }

    if found_id.is_empty() {
        println!("错误: 文件不存在");
        return Ok(());
    }

    // 家庭云不支持重命名文件夹
    if is_dir {
        println!("错误: 家庭云不支持重命名文件夹");
        return Ok(());
    }

    let url = "https://yun.139.com/orchestration/familyCloud-rebuild/photoContent/v1.0/modifyContentInfo";
    
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

    if resp.get("result").and_then(|r| r.get("resultCode")).and_then(|c| c.as_str()) == Some("0") {
        println!("重命名成功: {}", new_name);
    } else {
        println!("重命名失败: {:?}", resp);
    }

    Ok(())
}

async fn rename_group(config: &crate::config::Config, source: &str, new_name: &str) -> Result<(), ClientError> {
    let source = source.trim_start_matches('/');
    let parent_path = std::path::Path::new(source);
    let parent_dir = parent_path.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
    let file_name = parent_path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    
    let catalog_id = if parent_dir.is_empty() { "0".to_string() } else { parent_dir.clone() };
    
    let url = "https://yun.139.com/orchestration/group-rebuild/content/v1.0/queryGroupContentList";
    
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
    
    if let Some(catalog_list) = list_resp.pointer("/data/getGroupContentResult/catalogList").and_then(|v| v.as_array()) {
        for cat in catalog_list {
            if cat.get("catalogName").and_then(|v| v.as_str()) == Some(&file_name) {
                is_dir = true;
                found_id = cat.get("catalogID").and_then(|v| v.as_str()).unwrap_or("").to_string();
                found_path = cat.get("path").and_then(|v| v.as_str()).unwrap_or("").to_string();
                break;
            }
        }
    }
    
    if !is_dir && found_id.is_empty() {
        if let Some(content_list) = list_resp.pointer("/data/getGroupContentResult/contentList").and_then(|v| v.as_array()) {
            for content in content_list {
                if content.get("contentName").and_then(|v| v.as_str()) == Some(&file_name) {
                    found_id = content.get("contentID").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    found_path = list_resp.pointer("/data/getGroupContentResult/parentCatalogID").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    break;
                }
            }
        }
    }

    if found_id.is_empty() {
        println!("错误: 文件不存在");
        return Ok(());
    }

    if is_dir {
        let url = "https://yun.139.com/orchestration/group-rebuild/catalog/v1.0/modifyGroupCatalog";

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

        if resp.get("result").and_then(|r| r.get("resultCode")).and_then(|c| c.as_str()) == Some("0") {
            println!("重命名成功: {}", new_name);
        } else {
            println!("重命名失败: {:?}", resp);
        }
    } else {
        let url = "https://yun.139.com/orchestration/group-rebuild/content/v1.0/modifyGroupContent";

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

        if resp.get("result").and_then(|r| r.get("resultCode")).and_then(|c| c.as_str()) == Some("0") {
            println!("重命名成功: {}", new_name);
        } else {
            println!("重命名失败: {:?}", resp);
        }
    }

    Ok(())
}
