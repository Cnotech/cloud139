use serde::Deserialize;
use crate::client::ClientError;
use crate::config::Config;
use crate::utils::crypto;

const KEY_HEX_1: &str = "73634235495062495331515373756c734e7253306c673d3d";
const KEY_HEX_2: &str = "7150714477323633586746674c337538";

pub async fn login(
    username: &str,
    password: &str,
    mail_cookies: &str,
    storage_type: &str,
) -> Result<Config, ClientError> {
    log::info!("Starting login for user: {}", username);

    let step1_result = step1_login(username, password, mail_cookies).await?;
    log::info!("Step 1 completed: got sid={}", step1_result.sid);

    let step2_result = step2_get_artifact(&step1_result.sid, mail_cookies).await?;
    log::info!("Step 2 completed: got dycpwd");

    let step3_result = step3_third_login(username, &step2_result.dycpwd).await?;
    log::info!("Step 3 completed: got authToken");

    let authorization = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        format!("pc:{}:{}", step3_result.account, step3_result.auth_token),
    );

    let config = Config {
        authorization,
        username: username.to_string(),
        password: password.to_string(),
        mail_cookies: mail_cookies.to_string(),
        storage_type: storage_type.to_string(),
        cloud_id: Some(step3_result.cloud_id),
        user_domain_id: Some(step3_result.user_domain_id),
        custom_upload_part_size: 0,
        report_real_size: true,
        use_large_thumbnail: false,
        personal_cloud_host: None,
        refresh_token: Some(step3_result.auth_token),
        token_expire_time: Some(chrono::Utc::now().timestamp_millis() + 30 * 24 * 60 * 60 * 1000),
        root_folder_id: None,
    };

    Ok(config)
}

async fn step1_login(
    username: &str,
    password: &str,
    mail_cookies: &str,
) -> Result<Step1Result, ClientError> {
    let hashed_password = crypto::sha1_hash(&format!("fetion.com.cn:{}", password));
    
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36 Edg/141.0.0.0")
        .redirect(reqwest::redirect::Policy::none())
        .build()?;

    let cguid = chrono::Utc::now().timestamp_millis().to_string();

    let params = [
        ("UserName", username),
        ("passOld", ""),
        ("auto", "on"),
        ("Password", &hashed_password),
        ("webIndexPagePwdLogin", "1"),
        ("pwdType", "1"),
        ("clientId", "1003"),
        ("authType", "2"),
    ];

    let referer = format!(
        "https://mail.10086.cn/default.html?&s=1&v=0&u={}&m=1&ec=S001&resource=indexLogin&clientid=1003&auto=on&cguid={}&mtime=45",
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, username),
        cguid
    );

    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7".parse().unwrap());
    headers.insert("Content-Type", "application/x-www-form-urlencoded".parse().unwrap());
    headers.insert("Cookie", mail_cookies.parse().unwrap());
    headers.insert(reqwest::header::REFERER, reqwest::header::HeaderValue::from_str(&referer).unwrap());

    let resp = client
        .post("https://mail.10086.cn/Login/Login.ashx")
        .headers(headers)
        .form(&params)
        .send()
        .await?;

    let status = resp.status();
    if !(status.is_redirection()) {
        return Err(ClientError::Other(format!("Login failed with status: {}", status)));
    }

    let location = resp.headers()
        .get("Location")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let full_url = format!("https://mail.10086.cn{}", location);
    let url = url::Url::parse(&full_url)
        .map_err(|e| ClientError::Other(e.to_string()))?;

    let sid = url.query_pairs().find(|(k, _)| k == "sid")
        .map(|(_, v)| v.to_string())
        .unwrap_or_default();

    let extracted_cguid = url.query_pairs().find(|(k, _)| k == "cguid")
        .map(|(_, v)| v.to_string())
        .unwrap_or(cguid);

    if sid.is_empty() {
        return Err(ClientError::Other("Failed to extract sid from login response".to_string()));
    }

    log::debug!("Extracted sid: {}, cguid: {}", sid, extracted_cguid);

    Ok(Step1Result { sid, cguid: extracted_cguid })
}

async fn step2_get_artifact(sid: &str, mail_cookies: &str) -> Result<Step2Result, ClientError> {
    let cguid = chrono::Utc::now().timestamp_millis().to_string();
    let url = format!(
        "https://smsrebuild1.mail.10086.cn/setting/s?func=umc:getArtifact&sid={}&cguid={}",
        sid, cguid
    );

    // 从 mail_cookies 中提取 RMKEY
    let rmkey = mail_cookies.split(';')
        .map(|s| s.trim())
        .find(|s| s.starts_with("RMKEY="))
        .ok_or_else(|| ClientError::Other("RMKEY not found in mail_cookies".to_string()))?;

    let client = reqwest::Client::builder()
        .user_agent("okhttp/4.12.0")
        .build()?;

    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("Host", "smsrebuild1.mail.10086.cn".parse().unwrap());
    headers.insert("Cookie", rmkey.parse().unwrap());
    headers.insert("Content-Type", "text/xml; charset=utf-8".parse().unwrap());
    headers.insert("Accept-Encoding", "gzip".parse().unwrap());

    let resp = client
        .get(&url)
        .headers(headers)
        .send()
        .await?;

    let body = resp.text().await?;
    
    #[derive(Deserialize)]
    struct ArtifactResp {
        #[serde(rename = "var")]
        var: ArtifactVar,
    }

    #[derive(Deserialize)]
    struct ArtifactVar {
        artifact: String,
    }

    let artifact_resp: ArtifactResp = serde_json::from_str(&body)
        .map_err(|e| ClientError::Other(format!("Failed to parse artifact response: {}", e)))?;

    Ok(Step2Result {
        dycpwd: artifact_resp.var.artifact,
    })
}

async fn step3_third_login(
    username: &str,
    dycpwd: &str,
) -> Result<Step3Result, ClientError> {
    let key1 = hex::decode(KEY_HEX_1).map_err(|e| ClientError::Other(e.to_string()))?;
    let key2 = hex::decode(KEY_HEX_2).map_err(|e| ClientError::Other(e.to_string()))?;

    let secinfo = crypto::sha1_hash(&format!("fetion.com.cn:{}", dycpwd)).to_uppercase();

    let request_body = serde_json::json!({
        "clientkey_decrypt": "l3TryM&Q+X7@dzwk)qP",
        "clienttype": "886",
        "cpid": "507",
        "dycpwd": dycpwd,
        "extInfo": {"ifOpenAccount": "0"},
        "loginMode": "0",
        "msisdn": username,
        "pintype": "13",
        "secinfo": secinfo,
        "version": "20250901",
    });

    let body_str = serde_json::to_string(&request_body)
        .map_err(|e| ClientError::Other(e.to_string()))?;

    let iv = vec![0u8; 16];
    let encrypted = crypto::aes_cbc_encrypt(body_str.as_bytes(), &key1, &iv)
        .map_err(|e| ClientError::Other(e.to_string()))?;

    let mut payload = iv.clone();
    payload.extend(encrypted);
    
    let payload_base64 = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        &payload,
    );

    let url = "https://user-njs.yun.139.com/user/thirdlogin";

    let client = reqwest::Client::builder()
        .user_agent("okhttp/3.12.2")
        .build()?;

    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("hcy-cool-flag", "1".parse().unwrap());
    headers.insert("x-huawei-channelSrc", "10246600".parse().unwrap());
    headers.insert("Content-Type", "text/plain;charset=UTF-8".parse().unwrap());
    headers.insert("Host", "user-njs.yun.139.com".parse().unwrap());
    headers.insert("Connection", "Keep-Alive".parse().unwrap());

    let resp = client
        .post(url)
        .headers(headers)
        .body(payload_base64)
        .send()
        .await?;

    let resp_body = resp.bytes().await?;

    let resp_str = String::from_utf8_lossy(&resp_body);
    
    let layer1_str = if resp_str.trim_start().starts_with('{') {
        #[derive(Deserialize)]
        struct DirectResp {
            #[serde(rename = "authToken")]
            auth_token: Option<String>,
            account: Option<String>,
            #[serde(rename = "userDomainId")]
            user_domain_id: Option<String>,
            #[serde(rename = "cloudID")]
            cloud_id: Option<String>,
            #[serde(rename = "return")]
            return_code: Option<String>,
        }

        if let Ok(direct_resp) = serde_json::from_str::<DirectResp>(&resp_str) {
            if let (Some(auth_token), Some(account), Some(user_domain_id), Some(cloud_id)) = 
                (direct_resp.auth_token, direct_resp.account, direct_resp.user_domain_id, direct_resp.cloud_id) {
                return Ok(Step3Result {
                    auth_token,
                    account,
                    cloud_id,
                    user_domain_id,
                });
            }
            if let Some(return_code) = direct_resp.return_code {
                if return_code != "0" {
                    return Err(ClientError::Other(format!("Login failed with code: {}", return_code)));
                }
            }
        }
        return Err(ClientError::Other(format!("Failed to parse response: {}", resp_str)));
    } else {
        let decrypted = crypto::aes_cbc_decrypt(&resp_body, &key1, &iv)
            .map_err(|e| ClientError::Other(e.to_string()))?;
        String::from_utf8(decrypted.clone())
            .map_err(|e| ClientError::Other(e.to_string()))?
    };

    #[derive(Deserialize)]
    struct Layer1Resp {
        data: String,
    }

    let layer1: Layer1Resp = serde_json::from_str(&layer1_str)
        .map_err(|e| ClientError::Other(format!("Failed to parse layer1: {} - {}", e, layer1_str)))?;

    let hex_inner = layer1.data;
    let hex_inner_bytes = hex::decode(&hex_inner)
        .map_err(|e| ClientError::Other(format!("Failed to decode hex_inner: {}", e)))?;

    let decrypted_final = crypto::aes_ecb_decrypt(&hex_inner_bytes, &key2)
        .map_err(|e| ClientError::Other(e.to_string()))?;
    let final_str = String::from_utf8(decrypted_final)
        .map_err(|e| ClientError::Other(e.to_string()))?;

    #[derive(Deserialize)]
    struct ThirdLoginResp {
        #[serde(rename = "authToken")]
        auth_token: String,
        account: String,
        #[serde(rename = "userDomainId")]
        user_domain_id: String,
        #[serde(rename = "cloudID")]
        cloud_id: String,
    }

    let login_resp: ThirdLoginResp = serde_json::from_str(&final_str)
        .map_err(|e| ClientError::Other(format!("Failed to parse response: {} - {}", e, final_str)))?;

    Ok(Step3Result {
        auth_token: login_resp.auth_token,
        account: login_resp.account,
        cloud_id: login_resp.cloud_id,
        user_domain_id: login_resp.user_domain_id,
    })
}

pub async fn refresh_token(config: &Config) -> Result<Config, ClientError> {
    log::info!("Refreshing token for user: {}", config.username);

    if let Err(e) = check_token_expiration(config) {
        log::warn!("Token may be expired or invalid: {}, attempting re-login", e);
        return re_login(config).await;
    }

    let refresh_token = config.refresh_token.as_ref()
        .ok_or(ClientError::TokenExpired)?;

    let url = "https://aas.caiyun.feixin.10086.cn/tellin/authTokenRefresh.do";

    let body = format!(
        r#"<root><token>{}</token><account>{}</account><clienttype>656</clienttype></root>"#,
        refresh_token, config.username
    );

    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .build()?;

    let mut req_headers = reqwest::header::HeaderMap::new();
    req_headers.insert("Content-Type", "application/xml;charset=UTF-8".parse().unwrap());
    req_headers.insert("Referer", "https://yun.139.com/".parse().unwrap());

    let resp = client
        .post(url)
        .headers(req_headers)
        .body(body)
        .send()
        .await?;

    let text = resp.text().await?;

    #[derive(Deserialize)]
    #[serde(rename = "root")]
    struct RefreshResp {
        #[serde(rename = "return")]
        return_code: String,
        #[allow(dead_code)]
        token: String,
        #[serde(rename = "accessToken")]
        access_token: String,
    }

    let refresh_resp: RefreshResp = serde_xml_rs::from_str(&text)
        .map_err(|e| ClientError::Other(format!("Failed to parse refresh response: {}", e)))?;

    if refresh_resp.return_code != "0" {
        log::warn!("Token refresh failed with code: {}, attempting re-login", refresh_resp.return_code);
        return re_login(config).await;
    }

    let authorization = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        format!("pc:{}:{}", config.username, refresh_resp.access_token),
    );

    let mut new_config = config.clone();
    new_config.authorization = authorization;
    new_config.refresh_token = Some(refresh_resp.access_token);
    new_config.token_expire_time = Some(chrono::Utc::now().timestamp_millis() + 30 * 24 * 60 * 60 * 1000);

    Ok(new_config)
}

fn check_token_expiration(config: &Config) -> Result<(), ClientError> {
    let auth_parts: Vec<&str> = config.authorization.split(':').collect();
    if auth_parts.len() < 3 {
        return Err(ClientError::Other("Invalid authorization format".to_string()));
    }
    
    let token_part = auth_parts[2];
    let token_parts: Vec<&str> = token_part.split('|').collect();
    
    if token_parts.len() < 4 {
        return Err(ClientError::Other("Invalid token format".to_string()));
    }
    
    let expiration = token_parts[3].parse::<i64>()
        .map_err(|_| ClientError::Other("Invalid expiration timestamp".to_string()))?;
    
    let now = chrono::Utc::now().timestamp_millis();
    let remaining = expiration - now;
    
    if remaining < 0 {
        return Err(ClientError::TokenExpired);
    }
    
    if remaining > 15 * 24 * 60 * 60 * 1000 {
        return Err(ClientError::Other("Token still valid for more than 15 days".to_string()));
    }
    
    Ok(())
}

async fn re_login(config: &Config) -> Result<Config, ClientError> {
    log::info!("Attempting re-login for user: {}", config.username);
    
    let new_config = login(
        &config.username,
        &config.password,
        &config.mail_cookies,
        &config.storage_type,
    ).await?;
    
    Ok(new_config)
}

struct Step1Result {
    sid: String,
    #[allow(dead_code)]
    cguid: String,
}

struct Step2Result {
    dycpwd: String,
}

struct Step3Result {
    auth_token: String,
    account: String,
    cloud_id: String,
    user_domain_id: String,
}
