use clap::Parser;
use crate::client::{Client, ClientError, StorageType};
use crate::models::BatchCopyResp;

#[derive(Parser, Debug)]
pub struct CpArgs {
    #[arg(help = "源文件路径")]
    pub source: String,

    #[arg(help = "目标目录")]
    pub target: String,

    #[arg(short, long, help = "合并复制（覆盖目标中的同名文件）")]
    pub merge: bool,
}

pub async fn execute(args: CpArgs) -> Result<(), ClientError> {
    let config = crate::config::Config::load().map_err(|e| ClientError::Config(e))?;
    let storage_type = config.storage_type();

    match storage_type {
        StorageType::PersonalNew => {
            cp_personal(&config, &args.source, &args.target, args.merge).await?;
        }
        StorageType::Family => {
            cp_family(&config, &args.source, &args.target).await?;
        }
        StorageType::Group => {
            println!("群组云复制暂未实现");
        }
    }

    Ok(())
}

async fn cp_personal(config: &crate::config::Config, source: &str, target: &str, merge: bool) -> Result<(), ClientError> {
    let source_id = crate::client::api::get_file_id_by_path(config, source).await?;
    if source_id.is_empty() {
        println!("错误: 无效的源文件路径");
        return Ok(());
    }

    let target_id = if target == "/" || target.is_empty() {
        "".to_string()
    } else {
        crate::client::api::get_file_id_by_path(config, target).await?
    };
    
    let mut config = config.clone();
    let host = crate::client::api::get_personal_cloud_host(&mut config).await?;
    let url = format!("{}/file/batchCopy", host);

    let body = serde_json::json!({
        "fileIds": [source_id],
        "toParentFileId": target_id,
        "merge": merge
    });

    let resp: BatchCopyResp = crate::client::api::personal_api_request(&config, &url, body, StorageType::PersonalNew).await?;

    if resp.base.success {
        if merge {
            println!("合并复制成功");
        } else {
            println!("复制成功");
        }
    } else {
        println!("复制失败: {}", resp.base.message);
    }

    Ok(())
}

async fn cp_family(config: &crate::config::Config, source: &str, target: &str) -> Result<(), ClientError> {
    let url = "https://yun.139.com/orchestration/familyCloud-rebuild/contentCatalog/v1.0/copyContentCatalog";

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

    println!("复制响应: {:?}", resp);
    Ok(())
}
