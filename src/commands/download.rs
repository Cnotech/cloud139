use clap::Parser;
use std::path::Path;
use crate::client::{ClientError, StorageType};
use crate::models::DownloadUrlResp;

#[derive(Parser, Debug)]
pub struct DownloadArgs {
    #[arg(help = "远程文件路径")]
    pub remote_path: String,

    #[arg(default_value = ".", help = "本地保存路径")]
    pub local_path: String,
}

pub async fn execute(args: DownloadArgs) -> Result<(), ClientError> {
    let config = crate::config::Config::load().map_err(|e| ClientError::Config(e))?;
    let storage_type = config.storage_type();

    match storage_type {
        StorageType::PersonalNew => {
            download_personal(&config, &args.remote_path, &args.local_path).await?;
        }
        StorageType::Family => {
            println!("家庭云下载暂未实现");
        }
        StorageType::Group => {
            println!("群组云下载暂未实现");
        }
    }

    Ok(())
}

async fn download_personal(
    config: &crate::config::Config,
    file_id: &str,
    local_path: &str,
) -> Result<(), ClientError> {
    let mut config = config.clone();
    let host = crate::client::api::get_personal_cloud_host(&mut config).await?;
    let url = format!("{}/file/getDownloadUrl", host);

    let body = serde_json::json!({
        "fileId": file_id,
    });

    let resp: DownloadUrlResp = crate::client::api::personal_api_request(&config, &url, body, StorageType::PersonalNew).await?;

    if !resp.base.success {
        println!("获取下载链接失败: {}", resp.base.message);
        return Ok(());
    }

    let download_url = resp.data.cdn_url.unwrap_or(resp.data.url);
    println!("下载链接: {}", download_url);

    let local_path_obj = Path::new(local_path);
    if local_path_obj.is_dir() {
        let file_name = resp.data.file_name
            .unwrap_or_else(|| {
                Path::new(file_id)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("download")
                    .to_string()
            });
        let file_path = local_path_obj.join(file_name);
        download_file(&download_url, &file_path).await?;
    } else {
        download_file(&download_url, local_path_obj).await?;
    }

    Ok(())
}

async fn download_file(url: &str, local_path: &Path) -> Result<(), ClientError> {
    println!("开始下载到: {:?}", local_path);

    let client = reqwest::Client::new();
    let response = client.get(url).send().await?;

    if let Some(parent) = local_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let total_size = response.content_length();
    println!("文件大小: {} bytes", total_size.unwrap_or(0));

    let mut file = std::fs::File::create(local_path)?;
    let mut downloaded: u64 = 0;
    let mut stream = response.bytes_stream();

    use futures_util::StreamExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        use std::io::Write;
        file.write_all(&chunk)?;
        downloaded += chunk.len() as u64;
        if let Some(total) = total_size {
            print!("\r下载进度: {}/{} ({:.1}%)", downloaded, total, downloaded as f64 / total as f64 * 100.0);
        }
    }

    println!("\n下载完成!");
    Ok(())
}
