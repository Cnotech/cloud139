use clap::Parser;
use crate::client::{Client, ClientError, StorageType};
use crate::models::{PersonalUploadResp, FamilyCreateFolderRequest};

#[derive(Parser, Debug)]
pub struct MkdirArgs {
    #[arg(help = "新目录名称")]
    pub name: String,

    #[arg(default_value = "/", help = "父目录路径")]
    pub parent: String,
}

pub async fn execute(args: MkdirArgs) -> Result<(), ClientError> {
    let config = crate::config::Config::load().map_err(|e| ClientError::Config(e))?;
    let storage_type = config.storage_type();

    match storage_type {
        StorageType::PersonalNew => {
            mkdir_personal(&config, &args.name, &args.parent).await?;
        }
        StorageType::Family => {
            mkdir_family(&config, &args.name, &args.parent).await?;
        }
        StorageType::Group => {
            println!("群组云创建目录暂未实现");
        }
    }

    Ok(())
}

async fn mkdir_personal(config: &crate::config::Config, name: &str, parent: &str) -> Result<(), ClientError> {
    let host = crate::client::api::get_personal_cloud_host(config).await?;
    let url = format!("{}/file/create", host);

    let parent_file_id = if parent == "/" || parent.is_empty() {
        "".to_string()
    } else {
        parent.to_string()
    };

    let body = serde_json::json!({
        "parentFileId": parent_file_id,
        "name": name,
        "type": "folder"
    });

    let client = Client::new(config.clone());
    let resp: PersonalUploadResp = client.api_request_post(&url, body).await?;

    if resp.base.success {
        println!("目录创建成功: {}", resp.data.file_name);
    } else {
        println!("创建失败: {}", resp.base.message);
    }

    Ok(())
}

async fn mkdir_family(config: &crate::config::Config, name: &str, parent: &str) -> Result<(), ClientError> {
    let url = "https://yun.139.com/orchestration/familyCloud-rebuild/cloudCatalog/v1.0/createCloudDoc";

    let catalog_id = if parent == "/" || parent.is_empty() {
        "0".to_string()
    } else {
        parent.to_string()
    };

    let body = FamilyCreateFolderRequest {
        catalog_name: name.to_string(),
        parent_catalog_id: catalog_id,
    };

    let client = Client::new(config.clone());
    let resp: serde_json::Value = client.api_request_post(url, serde_json::to_value(body)?).await?;

    println!("创建目录响应: {:?}", resp);
    Ok(())
}
