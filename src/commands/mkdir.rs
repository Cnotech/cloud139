use crate::client::{Client, ClientError, StorageType};
use crate::models::PersonalUploadResp;
use crate::{success, error, info, warn};
use clap::Parser;

#[derive(Parser, Debug)]
pub struct MkdirArgs {
    #[arg(help = "新目录路径，格式: /父目录/新目录名")]
    pub path: String,

    #[arg(short, long, help = "强制继续，如果云端存在同名目录则自动重命名")]
    pub force: bool,
}

pub async fn execute(args: MkdirArgs) -> Result<(), ClientError> {
    let (parent, name) = parse_path(&args.path)?;

    let config = crate::config::Config::load().map_err(ClientError::Config)?;
    let storage_type = config.storage_type();

    match storage_type {
        StorageType::PersonalNew => {
            mkdir_personal(&config, &name, &parent, args.force).await?;
        }
        StorageType::Family => {
            mkdir_family(&config, &name, &parent).await?;
        }
        StorageType::Group => {
            mkdir_group(&config, &name, &parent).await?;
        }
    }

    Ok(())
}

pub fn parse_path(path: &str) -> Result<(String, String), ClientError> {
    let path = path.trim();
    if path.is_empty() {
        return Err(ClientError::Other("路径不能为空".to_string()));
    }

    let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    
    if parts.is_empty() || (parts.len() == 1 && parts[0].is_empty()) {
        return Err(ClientError::Other("无效的路径".to_string()));
    }

    let name = parts.last().unwrap().to_string();
    
    let parent = if parts.len() == 1 {
        "/".to_string()
    } else {
        let parent_parts = &parts[..parts.len() - 1];
        format!("/{}", parent_parts.join("/"))
    };

    Ok((parent, name))
}

async fn mkdir_personal(
    config: &crate::config::Config,
    name: &str,
    parent: &str,
    force: bool,
) -> Result<(), ClientError> {
    let _full_path = if parent == "/" || parent.is_empty() {
        format!("/{}", name)
    } else {
        format!("{}/{}", parent, name)
    };

    let mut config = config.clone();
    let host = crate::client::api::get_personal_cloud_host(&mut config).await?;
    let url = format!("{}/file/create", host);

    let parent_file_id = if parent == "/" || parent.is_empty() {
        "/".to_string()
    } else {
        crate::client::api::get_file_id_by_path(&config, parent).await?
    };

    if !force {
        let exists = crate::client::api::check_file_exists(&config, &parent_file_id, name).await?;
        if exists {
            warn!("云端已存在「{}」，如果继续则云端会自动进行重命名", name);
            error!("请使用 --force 参数确认继续");
            return Ok(());
        }
    }

    let body = serde_json::json!({
        "parentFileId": parent_file_id,
        "name": name,
        "description": "",
        "type": "folder",
        "fileRenameMode": "force_rename"
    });

    let resp: PersonalUploadResp =
        crate::client::api::personal_api_request(&config, &url, body, StorageType::PersonalNew)
            .await?;

    if resp.base.success {
        success!(
            "目录创建成功: {}",
            resp.data.map(|d| d.file_name.unwrap_or_default()).unwrap_or_default()
        );
    } else {
        error!(
            "创建失败: {}",
            resp.base.message.as_deref().unwrap_or("未知错误")
        );
    }

    Ok(())
}

async fn mkdir_family(
    config: &crate::config::Config,
    name: &str,
    parent: &str,
) -> Result<(), ClientError> {
    let url =
        "https://yun.139.com/orchestration/familyCloud-rebuild/cloudCatalog/v1.0/createCloudDoc";

    let _parent_id = if parent == "/" || parent.is_empty() {
        "0".to_string()
    } else {
        parent.to_string()
    };

    let path_value = if parent == "/" || parent.is_empty() {
        if let Some(ref root_path) = config.root_folder_id {
            root_path.clone()
        } else {
            "0".to_string()
        }
    } else {
        parent.to_string()
    };

    let body = serde_json::json!({
        "cloudID": config.cloud_id,
        "commonAccountInfo": {
            "account": config.account,
            "accountType": 1
        },
        "docLibName": name,
        "path": path_value
    });

    let client = Client::new(config.clone());
    let resp: serde_json::Value = client.api_request_post(url, body).await?;

    info!("创建目录响应: {:?}", resp);
    Ok(())
}

async fn mkdir_group(
    config: &crate::config::Config,
    name: &str,
    parent: &str,
) -> Result<(), ClientError> {
    let url = "https://yun.139.com/orchestration/group-rebuild/catalog/v1.0/createGroupCatalog";

    let parent_file_id = if parent == "/" || parent.is_empty() {
        "0".to_string()
    } else {
        parent.to_string()
    };

    let parent_path = if parent == "/" || parent.is_empty() {
        "root:".to_string()
    } else {
        let parent = parent.trim_start_matches('/');
        let parts: Vec<&str> = parent.split('/').collect();
        if parts.len() == 1 {
            format!("root:{}", parent)
        } else {
            let parent_name = parts[..parts.len() - 1].join("/");
            format!("root:/{}", parent_name)
        }
    };

    let body = serde_json::json!({
        "catalogName": name,
        "parentFileId": parent_file_id,
        "groupID": config.cloud_id,
        "commonAccountInfo": {
            "account": config.account,
            "accountType": 1
        },
        "path": parent_path
    });

    let client = Client::new(config.clone());
    let resp: serde_json::Value = client.api_request_post(url, body).await?;

    if resp
        .get("result")
        .and_then(|r| r.get("resultCode"))
        .and_then(|c| c.as_str())
        == Some("0")
    {
        success!("目录创建成功: {}", name);
    } else {
        error!("创建失败: {:?}", resp);
    }

    Ok(())
}
