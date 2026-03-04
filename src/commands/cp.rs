use clap::Parser;
use crate::client::{Client, ClientError, StorageType};
use crate::models::BatchCopyResp;

#[derive(Parser, Debug)]
pub struct CpArgs {
    #[arg(help = "源文件路径")]
    pub source: String,

    #[arg(help = "目标目录")]
    pub target: String,
}

pub async fn execute(args: CpArgs) -> Result<(), ClientError> {
    let config = crate::config::Config::load().map_err(|e| ClientError::Config(e))?;
    let storage_type = config.storage_type();

    match storage_type {
        StorageType::PersonalNew => {
            cp_personal(&config, &args.source, &args.target).await?;
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

async fn cp_personal(config: &crate::config::Config, source: &str, target: &str) -> Result<(), ClientError> {
    let mut config = config.clone();
    let host = crate::client::api::get_personal_cloud_host(&mut config).await?;
    let url = format!("{}/file/batchCopy", host);

    let body = serde_json::json!({
        "fileIds": [source],
        "toParentFileId": target
    });

    let resp: BatchCopyResp = crate::client::api::personal_api_request(&config, &url, body, StorageType::PersonalNew).await?;

    if resp.base.success {
        println!("复制成功");
    } else {
        println!("复制失败: {}", resp.base.message);
    }

    Ok(())
}

async fn cp_family(config: &crate::config::Config, source: &str, target: &str) -> Result<(), ClientError> {
    let url = "https://yun.139.com/orchestration/familyCloud-rebuild/contentCatalog/v1.0/copyContentCatalog";

    let body = serde_json::json!({
        "contentID": source,
        "targetCatalogID": target,
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
