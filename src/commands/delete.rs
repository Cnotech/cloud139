use clap::Parser;
use crate::client::{Client, ClientError, StorageType};
use crate::models::BatchTrashResp;

#[derive(Parser, Debug)]
pub struct DeleteArgs {
    #[arg(help = "远程文件路径")]
    pub path: String,

    #[arg(short, long, help = "确认删除")]
    pub force: bool,

    #[arg(short, long, help = "永久删除（不移动到回收站）")]
    pub permanent: bool,
}

pub async fn execute(args: DeleteArgs) -> Result<(), ClientError> {
    if !args.force {
        if args.permanent {
            println!("警告: 此操作将永久删除文件，无法恢复！");
        } else {
            println!("警告: 此操作会将文件移动到回收站");
        }
        println!("使用 --force 参数确认删除");
        return Ok(());
    }

    let config = crate::config::Config::load().map_err(|e| ClientError::Config(e))?;
    let storage_type = config.storage_type();

    match storage_type {
        StorageType::PersonalNew => {
            delete_personal(&config, &args.path, args.permanent).await?;
        }
        StorageType::Family => {
            delete_family(&config, &args.path).await?;
        }
        StorageType::Group => {
            delete_group(&config, &args.path, args.permanent).await?;
        }
    }

    Ok(())
}

async fn delete_personal(config: &crate::config::Config, path: &str, permanent: bool) -> Result<(), ClientError> {
    if path == "/" || path.is_empty() {
        println!("错误: 不能删除根目录");
        return Ok(());
    }

    let file_id = crate::client::api::get_file_id_by_path(config, path).await?;
    if file_id.is_empty() {
        println!("错误: 无效的文件路径");
        return Ok(());
    }

    let mut config = config.clone();
    let host = crate::client::api::get_personal_cloud_host(&mut config).await?;
    
    let url = if permanent {
        format!("{}/file/batchDelete", host)
    } else {
        format!("{}/recyclebin/batchTrash", host)
    };

    let body = serde_json::json!({
        "fileIds": [file_id]
    });

    let resp: BatchTrashResp = crate::client::api::personal_api_request(&config, &url, body, StorageType::PersonalNew).await?;

    if resp.base.success {
        if permanent {
            println!("文件已永久删除");
        } else {
            println!("文件已移动到回收站");
        }
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

async fn delete_group(config: &crate::config::Config, path: &str, permanent: bool) -> Result<(), ClientError> {
    if path == "/" || path.is_empty() {
        println!("错误: 不能删除根目录");
        return Ok(());
    }

    let catalog_id = if path.starts_with('/') || path.contains('/') {
        crate::client::api::get_file_id_by_path(config, path).await?
    } else {
        path.to_string()
    };

    let task_type = if permanent { 3 } else { 2 };
    let url = "https://yun.139.com/orchestration/group-rebuild/batchOprTask/v1.0/createBatchOprTask";

    let body = serde_json::json!({
        "taskType": task_type,
        "contentList": [{
            "contentID": catalog_id,
            "path": ""
        }],
        "sourceCloudID": config.cloud_id,
        "sourceCatalogType": 1003,
        "commonAccountInfo": {
            "account": config.username,
            "accountType": 1
        }
    });

    let client = Client::new(config.clone());
    let resp: serde_json::Value = client.api_request_post(url, body).await?;

    if resp.get("result").and_then(|r| r.get("resultCode")).and_then(|c| c.as_str()) == Some("0") {
        if permanent {
            println!("文件已永久删除");
        } else {
            println!("文件已移动到回收站");
        }
    } else {
        println!("删除失败: {:?}", resp);
    }

    Ok(())
}
