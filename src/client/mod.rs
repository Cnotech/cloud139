pub mod api;
pub mod api_trait;
pub mod auth;

use reqwest::header::HeaderValue;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::info;

const KEY_HEX_1: &str = "73634235495062495331515373756c734e7253306c673d3d";

pub const MCLOUD_VERSION: &str = "7.14.0";
pub const MCLOUD_CLIENT: &str = "10701";
pub const MCLOUD_CHANNEL: &str = "1000101";
pub const MCLOUD_CHANNEL_SRC: &str = "10000034";
pub const DEVICE_INFO: &str = "||9|7.14.0|chrome|120.0.0.0|||windows 10||zh-CN|||";
pub const CLIENT_INFO: &str = "||9|7.14.0|chrome|120.0.0.0|||windows 10||zh-CN|||dW5kZWZpbmVk||";

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
    #[error("无效的请求头: {0}")]
    InvalidHeader(String),
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
            .expect("Failed to build HTTP client");

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

        let headers = self.build_headers(&ts, &rand_str, &sign)?;

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

    fn build_headers(
        &self,
        ts: &str,
        rand_str: &str,
        sign: &str,
    ) -> Result<reqwest::header::HeaderMap, ClientError> {
        use reqwest::header::{HeaderMap, HeaderValue};
        let mut headers = HeaderMap::new();

        headers.insert(
            "Accept",
            HeaderValue::from_static("application/json, text/plain, */*"),
        );
        headers.insert("Caller", HeaderValue::from_static("web"));
        headers.insert("CMS-DEVICE", HeaderValue::from_static("default"));
        headers.insert("mcloud-channel", HeaderValue::from_static(MCLOUD_CHANNEL));
        headers.insert("mcloud-client", HeaderValue::from_static(MCLOUD_CLIENT));
        headers.insert("mcloud-route", HeaderValue::from_static("001"));
        headers.insert("mcloud-version", HeaderValue::from_static(MCLOUD_VERSION));
        headers.insert("Origin", HeaderValue::from_static("https://yun.139.com"));
        headers.insert("Referer", HeaderValue::from_static("https://yun.139.com/w/"));
        headers.insert("x-DeviceInfo", HeaderValue::from_static(DEVICE_INFO));
        headers.insert("x-huawei-channelSrc", HeaderValue::from_static(MCLOUD_CHANNEL_SRC));
        headers.insert("x-inner-ntwk", HeaderValue::from_static("2"));
        headers.insert("x-m4c-caller", HeaderValue::from_static("PC"));
        headers.insert("x-m4c-src", HeaderValue::from_static("10002"));
        headers.insert("x-yun-api-version", HeaderValue::from_static("v1"));
        headers.insert("x-yun-app-channel", HeaderValue::from_static(MCLOUD_CHANNEL_SRC));
        headers.insert("x-yun-channel-source", HeaderValue::from_static(MCLOUD_CHANNEL_SRC));
        headers.insert("x-yun-client-info", HeaderValue::from_static(CLIENT_INFO));
        headers.insert("x-yun-module-type", HeaderValue::from_static("100"));
        headers.insert("x-yun-svc-type", HeaderValue::from_static("1"));
        headers.insert("Inner-Hcy-Router-Https", HeaderValue::from_static("1"));

        headers.insert(
            "Authorization",
            format!("Basic {}", self.config.authorization)
                .parse()
                .map_err(|e| ClientError::InvalidHeader(format!("{}", e)))?,
        );
        headers.insert(
            "mcloud-sign",
            format!("{},{},{}", ts, rand_str, sign)
                .parse()
                .map_err(|e| ClientError::InvalidHeader(format!("{}", e)))?,
        );
        headers.insert(
            "x-SvcType",
            self.config
                .storage_type()
                .svc_type()
                .parse()
                .map_err(|e| ClientError::InvalidHeader(format!("{}", e)))?,
        );

        Ok(headers)
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

        let headers = self.build_and_album_headers()?;

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
            reqwest::header::HeaderValue::from_static("application/json, text/plain, */*"),
        );
        headers.insert(
            "Authorization",
            reqwest::header::HeaderValue::from_str(&format!("Basic {}", self.config.authorization))
                .map_err(|e| ClientError::InvalidHeader(e.to_string()))?,
        );
        headers.insert(
            "Content-Type",
            reqwest::header::HeaderValue::from_static("application/json;charset=UTF-8"),
        );
        headers.insert("mcloud-channel", reqwest::header::HeaderValue::from_static("1000101"));
        headers.insert("mcloud-client", reqwest::header::HeaderValue::from_static("10701"));
        headers.insert(
            "mcloud-sign",
            reqwest::header::HeaderValue::from_str(&format!("{},{},{}", ts, rand_str, sign))
                .map_err(|e| ClientError::InvalidHeader(e.to_string()))?,
        );
        headers.insert("mcloud-version", reqwest::header::HeaderValue::from_static("7.14.0"));
        headers.insert("Origin", reqwest::header::HeaderValue::from_static("https://yun.139.com"));
        headers.insert("Referer", reqwest::header::HeaderValue::from_static("https://yun.139.com/w/"));
        headers.insert(
            "x-DeviceInfo",
            reqwest::header::HeaderValue::from_static("||9|7.14.0|chrome|120.0.0.0|||windows 10||zh-CN|||"),
        );
        headers.insert("x-huawei-channelSrc", reqwest::header::HeaderValue::from_static("10000034"));
        headers.insert("x-inner-ntwk", reqwest::header::HeaderValue::from_static("2"));
        headers.insert("x-m4c-caller", reqwest::header::HeaderValue::from_static("PC"));
        headers.insert("x-m4c-src", reqwest::header::HeaderValue::from_static("10002"));
        headers.insert("x-SvcType", reqwest::header::HeaderValue::from_static("2"));
        headers.insert("Inner-Hcy-Router-Https", reqwest::header::HeaderValue::from_static("1"));

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

    fn build_and_album_headers(&self) -> Result<reqwest::header::HeaderMap, ClientError> {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("Host", HeaderValue::from_static("group.yun.139.com"));
        let auth_value = format!("Basic {}", self.config.authorization);
        let auth_header: reqwest::header::HeaderValue = auth_value
            .parse()
            .map_err(|e: reqwest::header::InvalidHeaderValue| ClientError::InvalidHeader(e.to_string()))?;
        headers.insert("authorization", auth_header);
        headers.insert("x-svctype", HeaderValue::from_static("2"));
        headers.insert("hcy-cool-flag", HeaderValue::from_static("1"));
        headers.insert("api-version", HeaderValue::from_static("v2"));
        headers.insert("x-huawei-channelsrc", HeaderValue::from_static("10246600"));
        headers.insert("x-sdk-channelsrc", HeaderValue::from_static(""));
        headers.insert("x-mm-source", HeaderValue::from_static("0"));
        headers.insert("x-deviceinfo", HeaderValue::from_static("1|127.0.0.1|1|12.3.2|Xiaomi|23116PN5BC||02-00-00-00-00-00|android 15|1440x3200|android|zh||||032|0|"));
        headers.insert(
            "content-type",
            HeaderValue::from_static("application/json; charset=utf-8"),
        );
        headers.insert("user-agent", HeaderValue::from_static("okhttp/4.11.0"));
        headers.insert("accept-encoding", HeaderValue::from_static("gzip"));
        Ok(headers)
    }
}
