use clap::Parser;
use std::path::Path;
use crate::client::{Client, ClientError, StorageType};
use crate::models::{UploadRequest, PersonalUploadResp, PersonalPartInfo};

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
    let host = crate::client::api::get_personal_cloud_host(config).await?;
    let url = format!("{}/file/create", host);

    println!("计算文件哈希...");
    let content_hash = crate::utils::crypto::calc_file_sha256(local_path.to_str().unwrap())?;

    let parent_file_id = if remote_path == "/" || remote_path.is_empty() {
        "".to_string()
    } else {
        remote_path.to_string()
    };

    let body = UploadRequest {
        content_hash: content_hash.clone(),
        content_hash_algorithm: "SHA256".to_string(),
        size: file_size,
        parent_file_id: parent_file_id.clone(),
        name: file_name.to_string(),
        file_rename_mode: Some("auto_rename".to_string()),
    };

    let client = Client::new(config.clone());
    let resp: PersonalUploadResp = client.api_request_post(&url, serde_json::to_value(body)?).await?;

    if !resp.base.success {
        println!("创建上传任务失败: {}", resp.base.message);
        return Ok(());
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
        println!("开始分片上传...");
        upload_parts(&host, local_path, &data.upload_id.unwrap(), &data.file_id, &part_infos, file_size).await?;
    }

    println!("上传完成: {}", data.file_name);
    Ok(())
}

async fn upload_parts(
    host: &str,
    local_path: &Path,
    upload_id: &str,
    file_id: &str,
    part_infos: &[PersonalPartInfo],
    _file_size: i64,
) -> Result<(), ClientError> {
    use std::fs::File;
    use std::io::{Read, Write};

    let mut file = File::open(local_path)?;
    let mut buffer = vec![0u8; 10 * 1024 * 1024]; // 10MB buffer

    for part in part_infos {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }

        let client = reqwest::Client::new();
        let resp = client
            .put(&part.upload_url)
            .body(buffer[..bytes_read].to_vec())
            .send()
            .await?;

        if !resp.status().is_success() {
            println!("分片 {} 上传失败", part.part_number);
        }

        print!("\r上传进度: {}/{}", part.part_number, part_infos.len());
    }

    println!("\n所有分片上传完成");

    let complete_url = format!("{}/file/complete", host);
    let client = reqwest::Client::new();

    let body = serde_json::json!({
        "uploadId": upload_id,
        "fileId": file_id,
    });

    let resp = client
        .post(&complete_url)
        .json(&body)
        .send()
        .await?;

    println!("完成响应: {:?}", resp.status());

    Ok(())
}
