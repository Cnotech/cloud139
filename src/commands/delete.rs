use clap::Parser;
use crate::client::{Client, ClientError, StorageType};
use crate::models::BatchTrashResp;

#[derive(Parser, Debug)]
pub struct DeleteArgs {
    #[arg(help = "远程文件路径")]
    pub path: String,

    #[arg(short, long, help = "确认删除")]
    pub force: bool,
}

pub async fn execute(args: DeleteArgs) -> Result<(), ClientError> {
    if !args.force {
        println!("警告: 此操作会将文件移动到回收站");
        println!("使用 --force 参数确认删除");
        return Ok(());
    }

    let config = crate::config::Config::load().map_err(|e| ClientError::Config(e))?;
    let storage_type = config.storage_type();

    match storage_type {
        StorageType::PersonalNew => {
            delete_personal(&config, &args.path).await?;
        }
        StorageType::Family => {
            delete_family(&config, &args.path).await?;
        }
        StorageType::Group => {
            println!("群组云删除暂未实现");
        }
    }

    Ok(())
}

async fn delete_personal(config: &crate::config::Config, file_id: &str) -> Result<(), ClientError> {
    let mut config = config.clone();
    let host = crate::client::api::get_personal_cloud_host(&mut config).await?;
    let url = format!("{}/recyclebin/batchTrash", host);

    let body = serde_json::json!({
        "fileIds": [file_id]
    });

    let resp: BatchTrashResp = crate::client::api::personal_api_request(&config, &url, body, StorageType::PersonalNew).await?;

    if resp.base.success {
        println!("文件已移动到回收站");
    } else {
        println!("删除失败: {}", resp.base.message);
    }

    Ok(())
}

async fn delete_family(config: &crate::config::Config, content_id: &str) -> Result<(), ClientError> {
    let url = "https://yun.139.com/orchestration/familyCloud-rebuild/batchOprTask/v1.0/createBatchOprTask";

    let body = serde_json::json!({
        "taskType": 2,
        "contentList": [{
            "contentID": content_id,
            "path": ""
        }],
        "sourceCloudID": config.cloud_id,
        "sourceCatalogType": 1002,
        "commonAccountInfo": {
            "account": config.username,
            "accountType": 1
        }
    });

    let client = Client::new(config.clone());
    let resp: serde_json::Value = client.api_request_post(url, body).await?;

    println!("删除响应: {:?}", resp);
    Ok(())
}
