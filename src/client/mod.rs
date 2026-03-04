pub mod auth;
pub mod api;

use serde::{Deserialize, Serialize};
use thiserror::Error;

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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StorageType {
    PersonalNew,
    Family,
    Group,
}

impl Default for StorageType {
    fn default() -> Self {
        Self::PersonalNew
    }
}

impl StorageType {
    pub fn as_str(&self) -> &'static str {
        match self {
            StorageType::PersonalNew => "personal_new",
            StorageType::Family => "family",
            StorageType::Group => "group",
        }
    }

    pub fn from_str(s: &str) -> Self {
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
        username: String,
        password: String,
        mail_cookies: String,
        storage_type: String,
    ) -> Result<Self, ClientError> {
        let config = auth::login(&username, &password, &mail_cookies, &storage_type).await?;
        config.save()?;
        Ok(Self::new(config))
    }

    pub async fn refresh_token_if_needed(&mut self) -> Result<(), ClientError> {
        if self.config.is_token_expired() {
            log::info!("Token expired, refreshing...");
            let new_config = auth::refresh_token(&self.config).await?;
            new_config.save()?;
            self.config = new_config;
        }
        Ok(())
    }

    pub async fn get_disk_info(&self) -> Result<(), ClientError> {
        let storage_type = self.config.storage_type();
        
        match storage_type {
            StorageType::PersonalNew => {
                let resp = api::get_personal_disk_info(&self.config).await?;
                if resp.base.success {
                    let data = resp.data;
                    println!("存储空间信息:");
                    println!("  总容量: {} GB", parse_size(&data.disk_size));
                    println!("  剩余: {} GB", parse_size(&data.free_disk_size));
                }
            }
            StorageType::Family => {
                let resp = api::get_family_disk_info(&self.config).await?;
                if resp.base.success {
                    let data = resp.data;
                    println!("家庭云存储空间信息:");
                    println!("  总容量: {} GB", parse_size(&data.disk_size));
                    println!("  已使用: {} GB", parse_size(&data.used_size));
                }
            }
            StorageType::Group => {
                println!("群组云存储信息查询暂未实现");
            }
        }
        
        Ok(())
    }

    pub async fn api_request_post<T: for<'de> Deserialize<'de>>(
        &self,
        url: &str,
        body: serde_json::Value,
    ) -> Result<T, ClientError> {
        let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let rand_str = generate_rand_str(6);
        let body_str = body.to_string();
        let sign = crate::utils::crypto::calc_sign(&body_str, &ts, &rand_str);

        let headers = self.build_headers(&ts, &rand_str, &sign);

        let resp = self.http_client
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
        headers.insert("Accept", "application/json, text/plain, */*".parse().unwrap());
        headers.insert("CMS-DEVICE", "default".parse().unwrap());
        headers.insert("Authorization", format!("Basic {}", self.config.authorization).parse().unwrap());
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
        headers.insert("x-SvcType", self.config.storage_type().svc_type().parse().unwrap());
        headers.insert("Inner-Hcy-Router-Https", "1".parse().unwrap());
        headers
    }
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

fn parse_size(size_str: &str) -> String {
    if let Ok(size) = size_str.parse::<i64>() {
        format!("{:.2}", size as f64 / 1024.0 / 1024.0 / 1024.0)
    } else {
        size_str.to_string()
    }
}
