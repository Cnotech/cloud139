pub mod api;
pub mod api_trait;
pub mod auth;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::info;

const KEY_HEX_1: &str = "73634235495062495331515373756c734e7253306c673d3d";

#[derive(Error, Debug)]
pub enum ClientError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("API error: {0}")]
    Api(String),
    #[error("Not logged in")]
    NotLoggedIn,
    #[error("Token expired")]
    TokenExpired,
    #[error("Config error: {0}")]
    Config(#[from] crate::config::ConfigError),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Other error: {0}")]
    Other(String),
    #[error("请使用 --force 参数确认继续")]
    ForceRequired,
    #[error("请使用 --yes 参数确认删除")]
    ConfirmationRequired,
    #[error("无效的源文件路径")]
    InvalidSourcePath,
    #[error("文件不存在")]
    FileNotFound,
    #[error("不能操作根目录")]
    CannotOperateOnRoot,
    #[error("没有有效的源文件需要处理")]
    NoSourceFiles,
    #[error("家庭云暂不支持批量移动")]
    UnsupportedFamilyBatchMove,
    #[error("群组云暂不支持批量移动")]
    UnsupportedGroupBatchMove,
    #[error("家庭云不支持重命名文件夹")]
    UnsupportedFamilyRenameFolder,
    #[error("不支持下载目录，请使用 ls 命令查看目录内容")]
    UnsupportedDownloadDirectory,
    #[error("无效的文件路径")]
    InvalidFilePath,
    #[error("操作被取消")]
    OperationCancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum StorageType {
    #[default]
    PersonalNew,
    Family,
    Group,
}

impl StorageType {
    pub fn as_str(&self) -> &'static str {
        match self {
            StorageType::PersonalNew => "personal_new",
            StorageType::Family => "family",
            StorageType::Group => "group",
        }
    }

    pub fn from_str_raw(s: &str) -> Self {
        match s {
            "family" => StorageType::Family,
            "group" => StorageType::Group,
            _ => StorageType::PersonalNew,
        }
    }

    pub fn svc_type(&self) -> &'static str {
        match self {
            StorageType::PersonalNew => "1",
            StorageType::Family => "2",
            StorageType::Group => "3",
        }
    }
}

pub struct Client {
    pub config: crate::config::Config,
    pub http_client: reqwest::Client,
}

impl Client {
    pub fn new(config: crate::config::Config) -> Self {
        let http_client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .build()
            .unwrap();

        Self {
            config,
            http_client,
        }
    }

    pub async fn login(
        token: String,
        storage_type: String,
        cloud_id: Option<String>,
    ) -> Result<Self, ClientError> {
        let config = auth::login(&token, &storage_type, cloud_id.as_deref()).await?;
        config.save()?;
        Ok(Self::new(config))
    }

    pub async fn refresh_token_if_needed(&mut self) -> Result<(), ClientError> {
        if self.config.is_token_expired() {
            info!("Token expired, refreshing...");
            let new_config = auth::refresh_token(&self.config).await?;
            new_config.save()?;
            self.config = new_config;
        }
        Ok(())
    }

    pub async fn api_request_post<T: for<'de> Deserialize<'de>>(
        &self,
        url: &str,
        body: serde_json::Value,
    ) -> Result<T, ClientError> {
        let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let rand_str = generate_rand_str(16);
        let body_str = body.to_string();
        let sign = crate::utils::crypto::calc_sign(&body_str, &ts, &rand_str);

        let headers = self.build_headers(&ts, &rand_str, &sign);

        let resp = self
            .http_client
            .post(url)
            .headers(headers)
            .json(&body)
            .send()
            .await?;

        let result: T = resp.json().await?;
        Ok(result)
    }

    fn build_headers(&self, ts: &str, rand_str: &str, sign: &str) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "Accept",
            "application/json, text/plain, */*".parse().unwrap(),
        );
        headers.insert("Caller", "web".parse().unwrap());
        headers.insert("CMS-DEVICE", "default".parse().unwrap());
        headers.insert(
            "Authorization",
            format!("Basic {}", self.config.authorization)
                .parse()
                .unwrap(),
        );
        headers.insert("mcloud-channel", "1000101".parse().unwrap());
        headers.insert("mcloud-client", "10701".parse().unwrap());
        headers.insert("mcloud-route", "001".parse().unwrap());
        headers.insert(
            "mcloud-sign",
            format!("{},{},{}", ts, rand_str, sign).parse().unwrap(),
        );
        headers.insert("mcloud-version", "7.14.0".parse().unwrap());
        headers.insert("Origin", "https://yun.139.com".parse().unwrap());
        headers.insert("Referer", "https://yun.139.com/w/".parse().unwrap());
        headers.insert(
            "x-DeviceInfo",
            "||9|7.14.0|chrome|120.0.0.0|||windows 10||zh-CN|||"
                .parse()
                .unwrap(),
        );
        headers.insert("x-huawei-channelSrc", "10000034".parse().unwrap());
        headers.insert("x-inner-ntwk", "2".parse().unwrap());
        headers.insert("x-m4c-caller", "PC".parse().unwrap());
        headers.insert("x-m4c-src", "10002".parse().unwrap());
        headers.insert(
            "x-SvcType",
            self.config.storage_type().svc_type().parse().unwrap(),
        );
        headers.insert("x-yun-api-version", "v1".parse().unwrap());
        headers.insert("x-yun-app-channel", "10000034".parse().unwrap());
        headers.insert("x-yun-channel-source", "10000034".parse().unwrap());
        headers.insert(
            "x-yun-client-info",
            "||9|7.14.0|chrome|120.0.0.0|||windows 10||zh-CN|||dW5kZWZpbmVk||"
                .parse()
                .unwrap(),
        );
        headers.insert("x-yun-module-type", "100".parse().unwrap());
        headers.insert("x-yun-svc-type", "1".parse().unwrap());
        headers.insert("Inner-Hcy-Router-Https", "1".parse().unwrap());
        headers
    }
}

pub fn generate_rand_str(len: usize) -> String {
    use rand::Rng;
    const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut rng = rand::thread_rng();
    (0..len)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

pub fn sort_json_value_to_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let pairs: Vec<String> = keys
                .iter()
                .map(|key| {
                    format!(
                        "{}:{}",
                        serde_json::to_string(key).unwrap_or_default(),
                        sort_json_value_to_string(&map[*key])
                    )
                })
                .collect();
            format!("{{{}}}", pairs.join(","))
        }
        serde_json::Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(sort_json_value_to_string).collect();
            format!("[{}]", items.join(","))
        }
        serde_json::Value::String(s) => {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(s) {
                sort_json_value_to_string(&parsed)
            } else {
                serde_json::to_string(s).unwrap_or_else(|_| s.clone())
            }
        }
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => "null".to_string(),
    }
}

impl Client {
    pub async fn and_album_request<T: for<'de> Deserialize<'de>>(
        &self,
        pathname: &str,
        body: serde_json::Value,
    ) -> Result<T, ClientError> {
        let url = format!(
            "https://group.yun.139.com/hcy/family/adapter/andAlbum/openApi{}",
            pathname
        );

        let headers = self.build_and_album_headers();

        let key1 = hex::decode(KEY_HEX_1).map_err(|e| ClientError::Other(e.to_string()))?;

        let sorted_body_str = sort_json_value_to_string(&body);

        let iv = vec![0u8; 16];
        let encrypted =
            crate::utils::crypto::aes_cbc_encrypt(sorted_body_str.as_bytes(), &key1, &iv)
                .map_err(|e| ClientError::Other(e.to_string()))?;

        let mut payload = iv.clone();
        payload.extend(encrypted);

        let payload_base64 =
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &payload);

        let resp = self
            .http_client
            .post(&url)
            .headers(headers)
            .body(payload_base64)
            .send()
            .await?;

        let resp_body = resp.bytes().await?;
        let resp_str = String::from_utf8_lossy(&resp_body);

        let decrypted = if resp_str.trim_start().starts_with('{') {
            resp_body.to_vec()
        } else {
            crate::utils::crypto::aes_cbc_decrypt(&resp_body, &key1, &iv)
                .map_err(|e| ClientError::Other(e.to_string()))?
        };

        let result: T = serde_json::from_slice(&decrypted)
            .map_err(|e| ClientError::Other(format!("Failed to parse response: {}", e)))?;

        Ok(result)
    }

    pub async fn isbo_post<T: for<'de> Deserialize<'de>>(
        &self,
        pathname: &str,
        body: serde_json::Value,
    ) -> Result<T, ClientError> {
        let url = format!("https://group.yun.139.com/hcy/mutual/adapter{}", pathname);

        let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let rand_str = generate_rand_str(16);
        let body_str = body.to_string();
        let sign = crate::utils::crypto::calc_sign(&body_str, &ts, &rand_str);

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "Accept",
            "application/json, text/plain, */*".parse().unwrap(),
        );
        headers.insert(
            "Authorization",
            format!("Basic {}", self.config.authorization)
                .parse()
                .unwrap(),
        );
        headers.insert(
            "Content-Type",
            "application/json;charset=UTF-8".parse().unwrap(),
        );
        headers.insert("mcloud-channel", "1000101".parse().unwrap());
        headers.insert("mcloud-client", "10701".parse().unwrap());
        headers.insert(
            "mcloud-sign",
            format!("{},{},{}", ts, rand_str, sign).parse().unwrap(),
        );
        headers.insert("mcloud-version", "7.14.0".parse().unwrap());
        headers.insert("Origin", "https://yun.139.com".parse().unwrap());
        headers.insert("Referer", "https://yun.139.com/w/".parse().unwrap());
        headers.insert(
            "x-DeviceInfo",
            "||9|7.14.0|chrome|120.0.0.0|||windows 10||zh-CN|||"
                .parse()
                .unwrap(),
        );
        headers.insert("x-huawei-channelSrc", "10000034".parse().unwrap());
        headers.insert("x-inner-ntwk", "2".parse().unwrap());
        headers.insert("x-m4c-caller", "PC".parse().unwrap());
        headers.insert("x-m4c-src", "10002".parse().unwrap());
        headers.insert("x-SvcType", "2".parse().unwrap());
        headers.insert("Inner-Hcy-Router-Https", "1".parse().unwrap());

        let resp = self
            .http_client
            .post(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await?;

        let result: T = resp.json().await?;
        Ok(result)
    }

    fn build_and_album_headers(&self) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("Host", "group.yun.139.com".parse().unwrap());
        headers.insert(
            "authorization",
            format!("Basic {}", self.config.authorization)
                .parse()
                .unwrap(),
        );
        headers.insert("x-svctype", "2".parse().unwrap());
        headers.insert("hcy-cool-flag", "1".parse().unwrap());
        headers.insert("api-version", "v2".parse().unwrap());
        headers.insert("x-huawei-channelsrc", "10246600".parse().unwrap());
        headers.insert("x-sdk-channelsrc", "".parse().unwrap());
        headers.insert("x-mm-source", "0".parse().unwrap());
        headers.insert("x-deviceinfo", "1|127.0.0.1|1|12.3.2|Xiaomi|23116PN5BC||02-00-00-00-00-00|android 15|1440x3200|android|zh||||032|0|".parse().unwrap());
        headers.insert(
            "content-type",
            "application/json; charset=utf-8".parse().unwrap(),
        );
        headers.insert("user-agent", "okhttp/4.11.0".parse().unwrap());
        headers.insert("accept-encoding", "gzip".parse().unwrap());
        headers
    }
}
