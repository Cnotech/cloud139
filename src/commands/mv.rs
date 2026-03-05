use clap::Parser;
use crate::client::{Client, ClientError, StorageType};
use crate::models::BatchMoveResp;

#[derive(Parser, Debug)]
pub struct MvArgs {
    #[arg(help = "源文件路径（支持多个，用空格分隔）")]
    pub source: Vec<String>,

    #[arg(help = "目标路径")]
    pub target: String,
}

pub async fn execute(args: MvArgs) -> Result<(), ClientError> {
    if args.source.is_empty() {
        println!("错误: 请指定至少一个源文件");
        return Ok(());
    }

    let config = crate::config::Config::load().map_err(|e| ClientError::Config(e))?;
    let storage_type = config.storage_type();

    match storage_type {
        StorageType::PersonalNew => {
            mv_personal(&config, &args.source, &args.target).await?;
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

async fn mv_personal(config: &crate::config::Config, sources: &[String], target: &str) -> Result<(), ClientError> {
    let target_normalized = if target == "/" || target.is_empty() {
        "/".to_string()
    } else {
        target.to_string()
    };

    let mut source_ids: Vec<String> = Vec::new();
    
    for source in sources {
        let source_path = std::path::Path::new(source);
        let source_parent = source_path.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
        
        let source_parent_normalized = if source_parent.is_empty() {
            "/".to_string()
        } else {
            source_parent
        };
        
        if source_parent_normalized == target_normalized {
            println!("警告: 源目录和目标目录相同，跳过: {}", source);
            continue;
        }

        let source_id = crate::client::api::get_file_id_by_path(config, source).await?;
        if source_id.is_empty() {
            println!("警告: 无效的源文件路径: {}", source);
            continue;
        }
        source_ids.push(source_id);
    }

    if source_ids.is_empty() {
        println!("错误: 没有有效的源文件需要移动");
        return Ok(());
    }

    let target_id = if target == "/" || target.is_empty() {
        "".to_string()
    } else {
        crate::client::api::get_file_id_by_path(config, target).await?
    };
    
    let mut config = config.clone();
    let host = crate::client::api::get_personal_cloud_host(&mut config).await?;
    let url = format!("{}/file/batchMove", host);

    let body = serde_json::json!({
        "fileIds": source_ids,
        "toParentFileId": target_id
    });

    let resp: BatchMoveResp = crate::client::api::personal_api_request(&config, &url, body, StorageType::PersonalNew).await?;

    if resp.base.success {
        println!("移动成功");
    } else {
        println!("移动失败: {}", resp.base.message);
    }

    Ok(())
}

async fn mv_family(config: &crate::config::Config, sources: &[String], target: &str) -> Result<(), ClientError> {
    if sources.len() > 1 {
        println!("家庭云暂不支持批量移动");
        return Ok(());
    }

    let source = &sources[0];
    let source_id = if source.starts_with('/') || source.contains('/') {
        crate::client::api::get_file_id_by_path(config, source).await?
    } else {
        source.to_string()
    };

    let target_id = if target.starts_with('/') || target.contains('/') {
        crate::client::api::get_file_id_by_path(config, target).await?
    } else {
        target.to_string()
    };

    let url = "https://yun.139.com/orchestration/familyCloud-rebuild/contentCatalog/v1.0/moveContentCatalog";

    let body = serde_json::json!({
        "contentID": source_id,
        "targetCatalogID": target_id,
        "cloudID": config.cloud_id,
        "commonAccountInfo": {
            "account": config.username,
            "accountType": 1
        }
    });

    let client = Client::new(config.clone());
    let resp: serde_json::Value = client.api_request_post(url, body).await?;

    println!("移动响应: {:?}", resp);
    Ok(())
}

async fn mv_group(config: &crate::config::Config, sources: &[String], target: &str) -> Result<(), ClientError> {
    if sources.len() > 1 {
        println!("群组云暂不支持批量移动");
        return Ok(());
    }

    let source = &sources[0];
    let source_id = if source.starts_with('/') || source.contains('/') {
        crate::client::api::get_file_id_by_path(config, source).await?
    } else {
        source.to_string()
    };

    let target_id = if target.starts_with('/') || target.contains('/') {
        crate::client::api::get_file_id_by_path(config, target).await?
    } else {
        target.to_string()
    };

    let url = "https://yun.139.com/orchestration/group-rebuild/contentCatalog/v1.0/moveGroupContent";

    let body = serde_json::json!({
        "contentID": source_id,
        "targetCatalogID": target_id,
        "cloudID": config.cloud_id,
        "commonAccountInfo": {
            "account": config.username,
            "accountType": 1
        }
    });

    let client = Client::new(config.clone());
    let resp: serde_json::Value = client.api_request_post(url, body).await?;

    if resp.get("result").and_then(|r| r.get("resultCode")).and_then(|c| c.as_str()) == Some("0") {
        println!("移动成功");
    } else {
        println!("移动失败: {:?}", resp);
    }

    Ok(())
}
