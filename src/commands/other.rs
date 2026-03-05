use clap::Parser;
use crate::client::{ClientError, StorageType};

#[derive(Parser, Debug)]
pub struct OtherArgs {
    #[arg(help = "操作类型")]
    pub action: String,

    #[arg(help = "文件路径")]
    pub path: Option<String>,
}

pub async fn execute(args: OtherArgs) -> Result<(), ClientError> {
    let config = crate::config::Config::load().map_err(|e| ClientError::Config(e))?;
    let _storage_type = config.storage_type();

    match args.action.as_str() {
        "video_preview" | "vp" => {
            if args.path.is_none() {
                println!("错误: 请指定文件路径");
                return Ok(());
            }
            video_preview(&config, args.path.as_ref().unwrap()).await?;
        }
        _ => {
            println!("未知操作: {}", args.action);
            println!("支持的操作为: video_preview (vp)");
        }
    }

    Ok(())
}

async fn video_preview(config: &crate::config::Config, path: &str) -> Result<(), ClientError> {
    let config = config.clone();
    let storage_type = config.storage_type();

    match storage_type {
        StorageType::PersonalNew => {
            video_preview_personal(&config, path).await?;
        }
        StorageType::Family => {
            println!("家庭云视频预览暂未支持");
        }
        StorageType::Group => {
            println!("群组云视频预览暂未支持");
        }
    }

    Ok(())
}

async fn video_preview_personal(config: &crate::config::Config, path: &str) -> Result<(), ClientError> {
    if path.is_empty() {
        println!("错误: 无效的文件路径");
        return Ok(());
    }

    let file_id = if path.chars().all(|c| c.is_ascii_digit()) {
        path.to_string()
    } else {
        crate::client::api::get_file_id_by_path(config, path).await?
    };

    if file_id.is_empty() {
        println!("错误: 无效的文件路径");
        return Ok(());
    }

    let mut config = config.clone();
    let host = crate::client::api::get_personal_cloud_host(&mut config).await?;
    let url = format!("{}/videoPreview/getPreviewInfo", host);

    let body = serde_json::json!({
        "category": "video",
        "fileId": file_id
    });

    #[derive(serde::Deserialize)]
    struct VideoPreviewResp {
        #[serde(flatten)]
        base: crate::models::BaseResp,
        data: Option<VideoPreviewData>,
    }

    #[derive(serde::Deserialize)]
    struct VideoPreviewData {
        url: String,
        #[serde(rename = "previewUrl")]
        preview_url: Option<String>,
    }

    let resp: VideoPreviewResp = crate::client::api::personal_api_request(&config, &url, body, StorageType::PersonalNew).await?;

    if resp.base.success {
        if let Some(data) = resp.data {
            println!("视频预览信息:");
            if let Some(preview_url) = data.preview_url {
                println!("  预览地址: {}", preview_url);
            }
            println!("  播放地址: {}", data.url);
        }
    } else {
        println!("获取视频预览失败: {}", resp.base.message);
    }

    Ok(())
}
