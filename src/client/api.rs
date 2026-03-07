use crate::client::ClientError;
use crate::config::Config;
use crate::models::QueryRoutePolicyResp;

pub async fn get_personal_cloud_host(config: &mut Config) -> Result<String, ClientError> {
    if let Some(ref host) = config.personal_cloud_host {
        return Ok(host.clone());
    }

    let url = "https://user-njs.yun.139.com/user/route/qryRoutePolicy";

    let body = serde_json::json!({
        "userInfo": {
            "userType": 1,
            "accountType": 1,
            "accountName": config.account
        },
        "modAddrType": 1
    });

    let client = reqwest::Client::new();

    let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let rand_str = generate_rand_str(16);
    let body_str = body.to_string();
    let sign = crate::utils::crypto::calc_sign(&body_str, &ts, &rand_str);

    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("Accept", "application/json, text/plain, */*".parse().unwrap());
    headers.insert("Authorization", format!("Basic {}", config.authorization).parse().unwrap());
    headers.insert("Content-Type", "application/json;charset=UTF-8".parse().unwrap());
    headers.insert("mcloud-channel", "1000101".parse().unwrap());
    headers.insert("mcloud-client", "10701".parse().unwrap());
    headers.insert("mcloud-sign", format!("{},{},{}", ts, rand_str, sign).parse().unwrap());
    headers.insert("mcloud-version", "7.14.0".parse().unwrap());
    headers.insert("Origin", "https://yun.139.com".parse().unwrap());
    headers.insert("Referer", "https://yun.139.com/w/".parse().unwrap());
    headers.insert("x-DeviceInfo", "||9|7.14.0|chrome|120.0.0.0|||windows 10||zh-CN|||".parse().unwrap());
    headers.insert("x-huawei-channelSrc", "10000034".parse().unwrap());
    headers.insert("x-inner-ntwk", "2".parse().unwrap());
    headers.insert("x-m4c-caller", "PC".parse().unwrap());
    headers.insert("x-m4c-src", "10002".parse().unwrap());
    headers.insert("x-SvcType", "1".parse().unwrap());
    headers.insert("Inner-Hcy-Router-Https", "1".parse().unwrap());

    let resp = client
        .post(url)
        .headers(headers)
        .json(&body)
        .send()
        .await?;

    let route_resp: QueryRoutePolicyResp = resp.json().await?;

    let host = route_resp.data.route_policy_list
        .into_iter()
        .find(|p| p.mod_name.as_deref() == Some("personal"))
        .map(|p| p.https_url.unwrap_or_default())
        .ok_or_else(|| ClientError::Other("Could not find personal cloud host".to_string()))?;

    config.personal_cloud_host = Some(host.clone());
    let _ = config.save();

    Ok(host)
}

pub async fn get_file_id_by_path(config: &Config, path: &str) -> Result<String, ClientError> {
    if path.is_empty() || path == "/" {
        return Ok(String::new());
    }

    let mut config = config.clone();
    let host = get_personal_cloud_host(&mut config).await?;

    let parts: Vec<&str> = path.trim_start_matches('/').split('/').filter(|s| !s.is_empty()).collect();
    
    let mut current_parent_id = String::new();
    
    for (i, part) in parts.iter().enumerate() {
        let is_last = i == parts.len() - 1;
        let parent_id = if current_parent_id.is_empty() {
            "/".to_string()
        } else {
            current_parent_id.clone()
        };
        
        let url = format!("{}/file/list", host);
        
        let body = serde_json::json!({
            "parentFileId": parent_id,
            "pageInfo": {
                "pageCursor": "",
                "pageSize": 100
            },
            "orderBy": "updated_at",
            "orderDirection": "DESC"
        });

        let list_resp: crate::models::PersonalListResp = personal_api_request(&config, &url, body, crate::client::StorageType::PersonalNew).await?;

        let items = list_resp.data.map(|d| d.items).unwrap_or_default();
        
        let target_id = items
            .into_iter()
            .find(|item| item.name.as_deref() == Some(part))
            .map(|item| item.file_id.unwrap_or_default());

        match target_id {
            Some(id) => {
                if is_last {
                    return Ok(id);
                }
                current_parent_id = id;
            }
            None => {
                return Err(ClientError::Api(format!("文件或目录不存在: {}", part)));
            }
        }
    }

    Ok(current_parent_id)
}

fn generate_rand_str(len: usize) -> String {
    use rand::Rng;
    const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut rng = rand::thread_rng();
    (0..len).map(|_| {
        let idx = rng.gen_range(0..CHARSET.len());
        CHARSET[idx] as char
    }).collect()
}

pub async fn personal_api_request<T: for<'de> serde::Deserialize<'de>>(
    config: &Config,
    url: &str,
    body: serde_json::Value,
    storage_type: crate::client::StorageType,
) -> Result<T, ClientError> {
    let svctype = match storage_type {
        crate::client::StorageType::PersonalNew => "1",
        crate::client::StorageType::Family => "2",
        crate::client::StorageType::Group => "3",
    };

    let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let rand_str = generate_rand_str(16);
    let body_str = body.to_string();
    let sign = crate::utils::crypto::calc_sign(&body_str, &ts, &rand_str);

    let client = reqwest::Client::new();

    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("Accept", "application/json, text/plain, */*".parse().unwrap());
    headers.insert("Authorization", format!("Basic {}", config.authorization).parse().unwrap());
    headers.insert("Caller", "web".parse().unwrap());
    headers.insert("CMS-DEVICE", "default".parse().unwrap());
    headers.insert("mcloud-channel", "1000101".parse().unwrap());
    headers.insert("mcloud-client", "10701".parse().unwrap());
    headers.insert("mcloud-route", "001".parse().unwrap());
    headers.insert("mcloud-sign", format!("{},{},{}", ts, rand_str, sign).parse().unwrap());
    headers.insert("mcloud-version", "7.14.0".parse().unwrap());
    headers.insert("x-DeviceInfo", "||9|7.14.0|chrome|120.0.0.0|||windows 10||zh-CN|||".parse().unwrap());
    headers.insert("x-huawei-channelSrc", "10000034".parse().unwrap());
    headers.insert("x-inner-ntwk", "2".parse().unwrap());
    headers.insert("x-m4c-caller", "PC".parse().unwrap());
    headers.insert("x-m4c-src", "10002".parse().unwrap());
    headers.insert("x-SvcType", svctype.parse().unwrap());
    headers.insert("x-yun-api-version", "v1".parse().unwrap());
    headers.insert("x-yun-app-channel", "10000034".parse().unwrap());
    headers.insert("x-yun-channel-source", "10000034".parse().unwrap());
    headers.insert("x-yun-client-info", "||9|7.14.0|chrome|120.0.0.0|||windows 10||zh-CN|||dW5kZWZpbmVk||".parse().unwrap());
    headers.insert("x-yun-module-type", "100".parse().unwrap());
    headers.insert("x-yun-svc-type", "1".parse().unwrap());
    headers.insert("Origin", "https://yun.139.com".parse().unwrap());
    headers.insert("Referer", "https://yun.139.com/w/".parse().unwrap());

    let resp = client
        .post(url)
        .headers(headers)
        .json(&body)
        .send()
        .await?;

    let result: T = resp.json().await?;
    Ok(result)
}

pub async fn check_file_exists(config: &Config, parent_file_id: &str, file_name: &str) -> Result<bool, ClientError> {
    let files = list_personal_files(config, parent_file_id).await?;
    Ok(files.iter().any(|f| f.name.as_deref() == Some(file_name)))
}

pub async fn list_personal_files(config: &Config, parent_file_id: &str) -> Result<Vec<crate::models::PersonalFileItem>, ClientError> {
    let mut config = config.clone();
    let host = get_personal_cloud_host(&mut config).await?;
    let url = format!("{}/file/list", host);

    let body = serde_json::json!({
        "imageThumbnailStyleList": ["Small", "Large"],
        "orderBy": "updated_at",
        "orderDirection": "DESC",
        "pageInfo": {
            "pageCursor": "",
            "pageSize": 100
        },
        "parentFileId": parent_file_id
    });

    let resp: crate::models::PersonalListResp = personal_api_request(&config, &url, body, crate::client::StorageType::PersonalNew).await?;

    Ok(resp.data.map(|d| d.items).unwrap_or_default())
}

pub async fn get_family_download_link(config: &Config, content_id: &str, path: &str) -> Result<String, ClientError> {
    let client = crate::client::Client::new(config.clone());
    
    let body = serde_json::json!({
        "contentID": content_id,
        "path": path,
        "catalogType": 3,
        "cloudID": config.cloud_id,
        "cloudType": 1,
        "commonAccountInfo": {
            "account": config.account,
            "accountType": 1
        }
    });

    let resp: serde_json::Value = client.api_request_post(
        "https://yun.139.com/orchestration/familyCloud-rebuild/content/v1.0/getFileDownLoadURL",
        body
    ).await?;

    let url = resp.pointer("/data/downloadURL")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    Ok(url)
}

pub async fn get_group_download_link(config: &Config, content_id: &str, path: &str) -> Result<String, ClientError> {
    let client = crate::client::Client::new(config.clone());
    
    let body = serde_json::json!({
        "contentID": content_id,
        "groupID": config.cloud_id,
        "path": path,
        "catalogType": 3,
        "cloudID": config.cloud_id,
        "cloudType": 1,
        "commonAccountInfo": {
            "account": config.account,
            "accountType": 1
        }
    });

    let resp: serde_json::Value = client.api_request_post(
        "https://yun.139.com/orchestration/group-rebuild/groupManage/v1.0/getGroupFileDownLoadURL",
        body
    ).await?;

    let url = resp.pointer("/data/downloadURL")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    Ok(url)
}

pub async fn get_family_root_path(config: &Config) -> Result<String, ClientError> {
    let client = crate::client::Client::new(config.clone());
    
    let body = serde_json::json!({
        "catalogID": "",
        "catalogType": 3,
        "cloudID": config.cloud_id,
        "cloudType": 1,
        "commonAccountInfo": {
            "account": config.account,
            "accountType": 1
        },
        "contentSortType": 0,
        "pageInfo": {
            "pageNum": 1,
            "pageSize": 1
        },
        "sortDirection": 1
    });

    let resp: serde_json::Value = client.api_request_post(
        "https://yun.139.com/orchestration/familyCloud-rebuild/content/v1.2/queryContentList",
        body
    ).await?;

    let path = resp.pointer("/data/path")
        .and_then(|v| v.as_str())
        .map(|s| {
            let s = s.trim_start_matches("root:/");
            let s = s.trim_start_matches("root:");
            s.to_string()
        })
        .unwrap_or_default();

    if path.is_empty() {
        if let Some(catalog_list) = resp.pointer("/data/cloudCatalogList").and_then(|v| v.as_array()) {
            if let Some(first) = catalog_list.first() {
                if let Some(p) = first.get("path").and_then(|v| v.as_str()) {
                    let p = p.trim_start_matches("root:/");
                    let p = p.trim_start_matches("root:");
                    return Ok(p.to_string());
                }
            }
        }
    }

    Ok(path)
}

pub async fn get_group_root_by_cloud_id(config: &Config) -> Result<String, ClientError> {
    let client = crate::client::Client::new(config.clone());
    
    let body = serde_json::json!({
        "groupID": config.cloud_id,
        "commonAccountInfo": {
            "account": config.account,
            "accountType": 1
        },
        "pageInfo": {
            "pageNum": 1,
            "pageSize": 1
        }
    });

    let resp: serde_json::Value = client.api_request_post(
        "https://yun.139.com/orchestration/group-rebuild/catalog/v1.0/queryGroupContentList",
        body
    ).await?;

    if let Some(parent_catalog_id) = resp.pointer("/data/getGroupContentResult/parentCatalogID")
        .and_then(|v| v.as_str()) 
    {
        if !parent_catalog_id.is_empty() {
            return Ok(parent_catalog_id.to_string());
        }
    }

    if let Some(catalog_list) = resp.pointer("/data/getGroupContentResult/catalogList")
        .and_then(|v| v.as_array()) 
    {
        if let Some(first) = catalog_list.first() {
            if let Some(p) = first.get("path").and_then(|v| v.as_str()) {
                return Ok(p.to_string());
            }
        }
    }

    Err(ClientError::Other("Failed to get group root path".to_string()))
}

pub async fn get_personal_download_link(config: &Config, file_id: &str) -> Result<String, ClientError> {
    let mut config = config.clone();
    let host = get_personal_cloud_host(&mut config).await?;
    let url = format!("{}/file/getDownloadUrl", host);

    let body = serde_json::json!({
        "fileId": file_id
    });

    let resp: serde_json::Value = personal_api_request(&config, &url, body, crate::client::StorageType::PersonalNew).await?;

    let cdn_url = resp.pointer("/data/cdnUrl")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if !cdn_url.is_empty() {
        return Ok(cdn_url);
    }

    let url = resp.pointer("/data/url")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    Ok(url)
}
