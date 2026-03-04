use serde::Deserialize;
use crate::client::ClientError;
use crate::config::Config;

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
    log::info!("Step 1 completed: got sid={}, cguid={}", step1_result.sid, step1_result.cguid);

    let step2_result = step2_get_artifact(&step1_result.sid, mail_cookies).await?;
    log::info!("Step 2 completed: got dycpwd");

    let step3_result = step3_third_login(username, &step1_result.sid, &step2_result.dycpwd).await?;
    log::info!("Step 3 completed: got authToken");

    let authorization = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        format!("pc:{}:{}", username, step3_result.auth_token),
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
    };

    Ok(config)
}

async fn step1_login(
    username: &str,
    password: &str,
    mail_cookies: &str,
) -> Result<Step1Result, ClientError> {
    let hashed_password = crate::utils::crypto::sha1_hash(&format!("fetion.com.cn:{}", password));
    
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .build()?;

    let params = [
        ("UserName", username),
        ("Password", &hashed_password),
        ("auto", "on"),
    ];

    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("Cookie", mail_cookies.parse().unwrap());
    headers.insert("Referer", "https://mail.10086.cn/".parse().unwrap());

    let resp = client
        .post("https://mail.10086.cn/Login/Login.ashx")
        .headers(headers)
        .form(&params)
        .send()
        .await?;

    let location = resp.headers()
        .get("Location")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let url = url::Url::parse(&format!("https://mail.10086.cn{}", location))
        .map_err(|e| ClientError::Other(e.to_string()))?;

    let sid = url.query_pairs().find(|(k, _)| k == "sid")
        .map(|(_, v)| v.to_string())
        .unwrap_or_default();

    let cguid = url.query_pairs().find(|(k, _)| k == "cguid")
        .map(|(_, v)| v.to_string())
        .unwrap_or_default();

    Ok(Step1Result { sid, cguid })
}

async fn step2_get_artifact(sid: &str, mail_cookies: &str) -> Result<Step2Result, ClientError> {
    let url = format!(
        "https://smsrebuild1.mail.10086.cn/setting/s?func=umc:getArtifact&sid={}",
        sid
    );

    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .build()?;

    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("Cookie", mail_cookies.parse().unwrap());
    headers.insert("Referer", "https://mail.10086.cn/".parse().unwrap());

    let resp = client
        .get(&url)
        .headers(headers)
        .send()
        .await?;

    #[derive(Deserialize)]
    struct ArtifactResp {
        #[serde(rename = "var")]
        var: ArtifactVar,
    }

    #[derive(Deserialize)]
    struct ArtifactVar {
        artifact: String,
    }

    let artifact_resp: ArtifactResp = resp.json().await?;

    Ok(Step2Result {
        dycpwd: artifact_resp.var.artifact,
    })
}

async fn step3_third_login(
    username: &str,
    sid: &str,
    dycpwd: &str,
) -> Result<Step3Result, ClientError> {
    let key1 = hex::decode(KEY_HEX_1).unwrap();
    let key2 = hex::decode(KEY_HEX_2).unwrap();

    let iv = vec![0u8; 16];

    let request_body = serde_json::json!({
        "msisdn": username,
        "dycpwd": dycpwd,
        "sid": sid,
        "clienttype": "886",
        "cpid": "507",
        "version": "7.14.0",
        "deviceid": "",
        "equipmenttype": "1",
    });

    let body_str = request_body.to_string();
    let encrypted = crate::utils::crypto::aes_cbc_encrypt(body_str.as_bytes(), &key1, &iv)
        .map_err(|e| ClientError::Other(e.to_string()))?;

    let url = "https://user-njs.yun.139.com/user/thirdlogin";

    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .build()?;

    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("Content-Type", "application/json;charset=UTF-8".parse().unwrap());
    headers.insert("Origin", "https://yun.139.com".parse().unwrap());
    headers.insert("Referer", "https://yun.139.com/".parse().unwrap());

    let encrypted_base64 = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        &encrypted,
    );

    let resp = client
        .post(url)
        .headers(headers)
        .body(encrypted_base64)
        .send()
        .await?;

    let encrypted_resp = resp.bytes().await?;

    let inner_hex = crate::utils::crypto::aes_cbc_decrypt(&encrypted_resp, &key1, &iv)
        .map_err(|e| ClientError::Other(e.to_string()))?;
    let inner_str = String::from_utf8(inner_hex.clone()).unwrap();
    let inner_hex_str = hex::encode(inner_hex);
    
    let decrypted = crate::utils::crypto::aes_cbc_decrypt(inner_hex_str.as_bytes(), &key2, &iv)
        .map_err(|e| ClientError::Other(e.to_string()))?;
    let final_str = String::from_utf8(decrypted).unwrap();

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
        cloud_id: login_resp.cloud_id,
        user_domain_id: login_resp.user_domain_id,
    })
}

pub async fn refresh_token(config: &Config) -> Result<Config, ClientError> {
    log::info!("Refreshing token for user: {}", config.username);

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
        token: String,
        #[serde(rename = "accessToken")]
        access_token: String,
    }

    let refresh_resp: RefreshResp = serde_xml_rs::from_str(&text)
        .map_err(|e| ClientError::Other(format!("Failed to parse refresh response: {}", e)))?;

    if refresh_resp.return_code != "0" {
        return Err(ClientError::TokenExpired);
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

struct Step1Result {
    sid: String,
    cguid: String,
}

struct Step2Result {
    dycpwd: String,
}

struct Step3Result {
    auth_token: String,
    cloud_id: String,
    user_domain_id: String,
}
