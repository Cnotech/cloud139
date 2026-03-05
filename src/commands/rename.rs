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
    let config = crate::config::Config::load().map_err(|e| ClientError::Config(e))?;
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
    let url = format!("{}/file/batchRename", host);

    let body = serde_json::json!({
        "fileIds": [file_id],
        "newName": new_name,
        "fileRenameMode": "force_rename"
    });

    let resp: crate::models::BatchRenameResp = crate::client::api::personal_api_request(&config, &url, body, StorageType::PersonalNew).await?;

    if resp.base.success {
        println!("重命名成功: {}", new_name);
    } else {
        println!("重命名失败: {}", resp.base.message);
    }

    Ok(())
}

async fn rename_family(config: &crate::config::Config, source: &str, new_name: &str) -> Result<(), ClientError> {
    let content_id = if source.starts_with('/') || source.contains('/') {
        crate::client::api::get_file_id_by_path(config, source).await?
    } else {
        source.to_string()
    };

    let url = "https://yun.139.com/orchestration/familyCloud-rebuild/contentCatalog/v1.0/renameContentCatalog";

    let body = serde_json::json!({
        "contentID": content_id,
        "newName": new_name,
        "cloudID": config.cloud_id,
        "commonAccountInfo": {
            "account": config.username,
            "accountType": 1
        }
    });

    let client = Client::new(config.clone());
    let resp: serde_json::Value = client.api_request_post(url, body).await?;

    if resp.get("result").and_then(|r| r.get("resultCode")).and_then(|c| c.as_str()) == Some("0") {
        println!("重命名成功: {}", new_name);
    } else {
        println!("重命名失败: {:?}", resp);
    }

    Ok(())
}

async fn rename_group(config: &crate::config::Config, source: &str, new_name: &str) -> Result<(), ClientError> {
    let catalog_id = if source.starts_with('/') || source.contains('/') {
        crate::client::api::get_file_id_by_path(config, source).await?
    } else {
        source.to_string()
    };

    let url = "https://yun.139.com/orchestration/group-rebuild/contentCatalog/v1.0/renameGroupCatalog";

    let body = serde_json::json!({
        "catalogID": catalog_id,
        "newName": new_name,
        "commonAccountInfo": {
            "account": config.username,
            "accountType": 1
        }
    });

    let client = Client::new(config.clone());
    let resp: serde_json::Value = client.api_request_post(url, body).await?;

    if resp.get("result").and_then(|r| r.get("resultCode")).and_then(|c| c.as_str()) == Some("0") {
        println!("重命名成功: {}", new_name);
    } else {
        println!("重命名失败: {:?}", resp);
    }

    Ok(())
}
