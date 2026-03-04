use clap::Parser;
use crate::client::{Client, ClientError, StorageType};
use crate::models::BatchMoveResp;

#[derive(Parser, Debug)]
pub struct MvArgs {
    #[arg(help = "源文件路径")]
    pub source: String,

    #[arg(help = "目标路径")]
    pub target: String,
}

pub async fn execute(args: MvArgs) -> Result<(), ClientError> {
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
            println!("群组云移动暂未实现");
        }
    }

    Ok(())
}

async fn mv_personal(config: &crate::config::Config, source: &str, target: &str) -> Result<(), ClientError> {
    let mut config = config.clone();
    let host = crate::client::api::get_personal_cloud_host(&mut config).await?;
    let url = format!("{}/file/batchMove", host);

    let body = serde_json::json!({
        "fileIds": [source],
        "toParentFileId": target
    });

    let resp: BatchMoveResp = crate::client::api::personal_api_request(&config, &url, body, StorageType::PersonalNew).await?;

    if resp.base.success {
        println!("移动成功");
    } else {
        println!("移动失败: {}", resp.base.message);
    }

    Ok(())
}

async fn mv_family(config: &crate::config::Config, source: &str, target: &str) -> Result<(), ClientError> {
    let url = "https://yun.139.com/orchestration/familyCloud-rebuild/contentCatalog/v1.0/moveContentCatalog";

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

    println!("移动响应: {:?}", resp);
    Ok(())
}
