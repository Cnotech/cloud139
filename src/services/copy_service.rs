use crate::client::endpoints::group;
use crate::client::{Client, ClientError};
use crate::domain::StorageType;
use crate::models::BatchCopyResp;
use crate::{debug, error, success, warn};

pub async fn cp(
    config: &crate::config::Config,
    sources: &[String],
    target: &str,
    force: bool,
) -> Result<(), ClientError> {
    if sources.is_empty() {
        error!("没有有效的源文件需要复制");
        return Err(ClientError::NoSourceFiles);
    }

    let storage_type = config.storage_type();
    match storage_type {
        StorageType::PersonalNew => cp_personal(config, sources, target, force).await?,
        StorageType::Family => cp_family(config, sources, target).await?,
        StorageType::Group => cp_group(config, sources, target).await?,
    }
    Ok(())
}

async fn cp_personal(
    config: &crate::config::Config,
    sources: &[String],
    target: &str,
    force: bool,
) -> Result<(), ClientError> {
    let mut source_ids: Vec<String> = Vec::new();
    let mut file_names: Vec<String> = Vec::new();

    for source in sources {
        let source_id = crate::client::api::get_file_id_by_path(config, source).await?;
        if source_id.is_empty() {
            if sources.len() == 1 {
                error!("无效的源文件路径");
                return Err(ClientError::InvalidSourcePath);
            }
            warn!("无效的源文件路径: {}", source);
            continue;
        }

        debug!("cp_personal: source={}, id={}", source, source_id);

        let source_path = std::path::Path::new(source);
        let file_name = source_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        source_ids.push(source_id);
        file_names.push(file_name);
    }

    if source_ids.is_empty() {
        error!("没有有效的源文件需要复制");
        return Err(ClientError::NoSourceFiles);
    }

    let target_id = if target == "/" || target.is_empty() {
        "/".to_string()
    } else {
        crate::client::api::get_file_id_by_path(config, target).await?
    };
    debug!("cp_personal: target_id={}", target_id);

    if !force {
        let mut seen_names = std::collections::HashSet::new();
        for file_name in &file_names {
            if !seen_names.insert(file_name.clone()) {
                warn!(
                    "复制列表中存在重名文件「{}」，继续时云端会自动重命名",
                    file_name
                );
                error!("请使用 --force 参数确认继续");
                return Err(ClientError::ForceRequired);
            }

            let exists =
                crate::client::api::check_file_exists(config, &target_id, file_name).await?;
            if exists {
                warn!(
                    "云端已存在「{}」，如果继续则云端会自动进行重命名",
                    file_name
                );
                error!("请使用 --force 参数确认继续");
                return Err(ClientError::ForceRequired);
            }
        }
    }

    let mut config = config.clone();
    let host = crate::client::api::get_personal_cloud_host(&mut config).await?;
    let url = format!("{}/file/batchCopy", host);

    let body = serde_json::json!({
        "fileIds": source_ids,
        "toParentFileId": target_id
    });

    let resp: BatchCopyResp =
        crate::client::api::personal_api_request(&config, &url, body, StorageType::PersonalNew)
            .await?;

    if resp.base.success {
        success!("复制成功");
    } else {
        let msg = resp.base.message.as_deref().unwrap_or("未知错误");
        error!("复制失败: {}", msg);
        return Err(ClientError::Api(msg.to_string()));
    }

    Ok(())
}

async fn cp_family(
    config: &crate::config::Config,
    sources: &[String],
    target: &str,
) -> Result<(), ClientError> {
    if sources.len() > 1 {
        error!("家庭云暂不支持批量复制");
        return Err(ClientError::UnsupportedFamilyBatchCopy);
    }

    let client = Client::new(config.clone());
    let source = &sources[0];

    let body = serde_json::json!({
        "commonAccountInfo": {
            "accountType": "1",
            "accountUserId": &config.account
        },
        "destCatalogID": target,
        "destCloudID": config.cloud_id,
        "sourceCatalogIDs": [],
        "sourceCloudID": config.cloud_id,
        "sourceContentIDs": [source]
    });

    let resp: serde_json::Value = client
        .and_album_request("/copyContentCatalog", body)
        .await?;

    debug!("复制响应: {:?}", resp);
    Ok(())
}

async fn cp_group(
    config: &crate::config::Config,
    sources: &[String],
    target: &str,
) -> Result<(), ClientError> {
    if sources.len() > 1 {
        error!("群组云暂不支持批量复制");
        return Err(ClientError::UnsupportedGroupBatchCopy);
    }

    let client = Client::new(config.clone());
    let source = &sources[0];

    let source = source.trim_start_matches('/');
    let target = target.trim_start_matches('/');

    let parent_path = std::path::Path::new(source);
    let parent_dir = parent_path
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let file_name = parent_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    let catalog_id = if parent_dir.is_empty() {
        "0".to_string()
    } else {
        parent_dir.clone()
    };

    let url = group::orchestration::QUERY_GROUP_CONTENT_LIST;

    let list_body = serde_json::json!({
        "groupID": config.cloud_id,
        "catalogID": catalog_id,
        "contentSortType": 0,
        "sortDirection": 1,
        "startNumber": 1,
        "endNumber": 100,
        "path": if parent_dir.is_empty() { "root:".to_string() } else { format!("root:/{}", parent_dir) }
    });

    let list_resp: serde_json::Value = client.api_request_post(url, list_body).await?;

    let mut is_dir = false;
    let mut found_id = String::new();
    let mut found_path = String::new();

    if let Some(catalog_list) = list_resp
        .pointer("/data/getGroupContentResult/catalogList")
        .and_then(|v| v.as_array())
    {
        for cat in catalog_list {
            if cat.get("catalogName").and_then(|v| v.as_str()) == Some(&file_name) {
                is_dir = true;
                found_id = cat
                    .get("catalogID")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                found_path = cat
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                break;
            }
        }
    }

    if !is_dir
        && found_id.is_empty()
        && let Some(content_list) = list_resp
            .pointer("/data/getGroupContentResult/contentList")
            .and_then(|v| v.as_array())
    {
        for content in content_list {
            if content.get("contentName").and_then(|v| v.as_str()) == Some(&file_name) {
                found_id = content
                    .get("contentID")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                found_path = list_resp
                    .pointer("/data/getGroupContentResult/parentCatalogID")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                break;
            }
        }
    }

    if found_id.is_empty() {
        error!("文件不存在");
        return Err(ClientError::FileNotFound);
    }

    let full_source_path = if found_path.is_empty() {
        format!("root:/{}", found_id)
    } else {
        format!("root:/{}/{}", found_path.trim_end_matches('/'), found_id)
    };

    let dest_catalog_id = if target.is_empty() {
        "root:".to_string()
    } else {
        format!("root:{}", target.trim_end_matches('/'))
    };

    let body = if is_dir {
        serde_json::json!({
            "commonAccountInfo": {
                "accountType": "1",
                "accountUserId": &config.account
            },
            "destCatalogID": dest_catalog_id,
            "destCloudID": config.cloud_id,
            "sourceCatalogIDs": [full_source_path],
            "sourceCloudID": config.cloud_id,
            "sourceContentIDs": []
        })
    } else {
        serde_json::json!({
            "commonAccountInfo": {
                "accountType": "1",
                "accountUserId": &config.account
            },
            "destCatalogID": dest_catalog_id,
            "destCloudID": config.cloud_id,
            "sourceCatalogIDs": [],
            "sourceCloudID": config.cloud_id,
            "sourceContentIDs": [full_source_path]
        })
    };

    let resp: serde_json::Value = client
        .and_album_request("/copyContentCatalog", body)
        .await?;

    if resp
        .get("result")
        .and_then(|r| r.get("resultCode"))
        .and_then(|c| c.as_str())
        == Some("0")
    {
        success!("复制成功");
    } else {
        error!("复制失败: {:?}", resp);
        return Err(ClientError::Api(format!("{:?}", resp)));
    }

    Ok(())
}
