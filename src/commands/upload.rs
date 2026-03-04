use clap::Parser;
use std::path::Path;
use crate::client::{ClientError, StorageType};
use crate::models::{UploadRequest, PersonalUploadResp};

#[derive(Parser, Debug)]
pub struct UploadArgs {
    #[arg(help = "本地文件路径")]
    pub local_path: String,

    #[arg(default_value = "/", help = "远程目录路径")]
    pub remote_path: String,
}

pub async fn execute(args: UploadArgs) -> Result<(), ClientError> {
    let config = crate::config::Config::load().map_err(|e| ClientError::Config(e))?;
    let storage_type = config.storage_type();

    let local_path = Path::new(&args.local_path);
    if !local_path.exists() {
        println!("错误: 文件不存在: {}", args.local_path);
        return Ok(());
    }

    let file_name = local_path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    let metadata = std::fs::metadata(local_path)?;
    let file_size = metadata.len() as i64;

    println!("上传文件: {} -> {}/{}", args.local_path, args.remote_path, file_name);
    println!("文件大小: {} bytes", file_size);

    match storage_type {
        StorageType::PersonalNew => {
            upload_personal(&config, local_path, &args.remote_path, file_name, file_size).await?;
        }
        StorageType::Family => {
            println!("家庭云上传暂未实现");
        }
        StorageType::Group => {
            println!("群组云上传暂未实现");
        }
    }

    Ok(())
}

async fn upload_personal(
    config: &crate::config::Config,
    local_path: &Path,
    remote_path: &str,
    file_name: &str,
    file_size: i64,
) -> Result<(), ClientError> {
    let mut config = config.clone();
    let host = crate::client::api::get_personal_cloud_host(&mut config).await?;
    let url = format!("{}/file/create", host);

    println!("计算文件哈希...");
    let content_hash = crate::utils::crypto::calc_file_sha256(local_path.to_str().unwrap())?;

    let parent_file_id = if remote_path == "/" || remote_path.is_empty() {
        "".to_string()
    } else {
        crate::client::api::get_file_id_by_path(&config, remote_path).await?
    };

    let body = UploadRequest {
        content_hash: content_hash.clone(),
        content_hash_algorithm: "SHA256".to_string(),
        size: file_size,
        parent_file_id: parent_file_id.clone(),
        name: file_name.to_string(),
        file_rename_mode: Some("auto_rename".to_string()),
        file_type: Some("file".to_string()),
    };

    let resp: PersonalUploadResp = crate::client::api::personal_api_request(&config, &url, serde_json::to_value(body)?, StorageType::PersonalNew).await?;

    if !resp.base.success {
        return Err(ClientError::Api(format!("创建上传任务失败: {}", resp.base.message)));
    }

    let data = resp.data;

    if data.exist {
        println!("文件已存在: {}", data.file_name);
        return Ok(());
    }

    if data.rapid_upload {
        println!("秒传成功: {}", data.file_name);
        return Ok(());
    }

    if let Some(part_infos) = data.part_infos {
        if part_infos.is_empty() {
            println!("服务器未返回分片信息，执行普通上传...");
        } else {
            println!("开始分片上传...");
            upload_parts(&host, local_path, &data.upload_id.unwrap(), &data.file_id, file_size, &content_hash).await?;
            println!("上传完成: {}", data.file_name);
            return Ok(());
        }
    } else {
        println!("服务器未返回分片信息，执行普通上传...");
    }

    Ok(())
}

async fn upload_parts(
    host: &str,
    local_path: &Path,
    upload_id: &str,
    file_id: &str,
    file_size: i64,
    content_hash: &str,
) -> Result<(), ClientError> {
    use std::fs::File;
    use std::io::{Read, Seek, SeekFrom};

    let mut file = File::open(local_path)?;
    let part_size: i64 = 100 * 1024 * 1024;
    let part_count = (file_size + part_size - 1) / part_size;

    let mut all_urls: Vec<String> = Vec::new();
    let mut current_url_index: i64 = 0;

    loop {
        let get_url = format!("{}/file/getUploadUrl", host);
        let client = reqwest::Client::new();
        
        let body = serde_json::json!({
            "uploadId": upload_id,
            "fileId": file_id,
            "partNumber": current_url_index + 1,
            "uploadedSize": (current_url_index * part_size) as i64
        });

        let resp = client
            .post(&get_url)
            .json(&body)
            .send()
            .await?;

        let resp_json: serde_json::Value = resp.json().await?;
        
        if let Some(url) = resp_json.get("data").and_then(|d| d.get("uploadUrl")).and_then(|u| u.as_str()) {
            all_urls.push(url.to_string());
            current_url_index += 1;
            if current_url_index >= part_count {
                break;
            }
        } else {
            let error_msg = resp_json.get("message").or(resp_json.get("base").and_then(|b| b.get("message"))).and_then(|m| m.as_str()).unwrap_or("获取上传URL失败");
            return Err(ClientError::Api(format!("获取分片上传URL失败: {}", error_msg)));
        }
    }

    if all_urls.is_empty() {
        return Err(ClientError::Api("未能获取任何分片上传URL".to_string()));
    }

    file.seek(SeekFrom::Start(0))?;

    for (i, upload_url) in all_urls.iter().enumerate() {
        file.seek(SeekFrom::Start(i as u64 * part_size as u64))?;
        
        let read_size = if (i as i64 + 1) * part_size > file_size {
            file_size - i as i64 * part_size
        } else {
            part_size
        };

        let mut buffer = vec![0u8; read_size as usize];
        let bytes_read = file.read(&mut buffer)?;
        
        if bytes_read == 0 {
            break;
        }

        let part_number = i + 1;
        println!("上传分片 {}/{}", part_number, part_count);

        let client = reqwest::Client::new();
        let resp = client
            .put(upload_url)
            .body(buffer)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(ClientError::Api(format!("分片 {} 上传失败: {}", part_number, resp.status())));
        }
    }

    println!("\n所有分片上传完成");

    let complete_url = format!("{}/file/complete", host);
    let client = reqwest::Client::new();

    let body = serde_json::json!({
        "contentHash": content_hash,
        "contentHashAlgorithm": "SHA256",
        "uploadId": upload_id,
        "fileId": file_id,
    });

    let resp = client
        .post(&complete_url)
        .json(&body)
        .send()
        .await?;

    let status = resp.status();
    let resp_json: serde_json::Value = resp.json().await?;
    
    if let Some(success) = resp_json.get("base").and_then(|b| b.get("success")).and_then(|s| s.as_bool()) {
        if success {
            println!("完成响应: {:?}", status);
        } else {
            let message = resp_json.get("base").and_then(|b| b.get("message")).and_then(|m| m.as_str()).unwrap_or("完成上传失败");
            return Err(ClientError::Api(format!("完成上传失败: {}", message)));
        }
    } else {
        println!("完成响应: {:?}", status);
    }

    Ok(())
}
