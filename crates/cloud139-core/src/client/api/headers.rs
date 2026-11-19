use crate::client::ClientError;
use reqwest::header::{CONTENT_LENGTH, CONTENT_TYPE, HeaderMap, HeaderValue, ORIGIN, REFERER};

pub const MCLOUD_VERSION: &str = "7.14.0";
pub const MCLOUD_CLIENT: &str = "10701";
pub const MCLOUD_CHANNEL: &str = "1000101";
pub const MCLOUD_CHANNEL_SRC: &str = "10000034";
pub const DEVICE_INFO: &str = "||9|7.14.0|chrome|120.0.0.0|||windows 10||zh-CN|||";
pub const PC_CLIENT_INFO: &str =
    "||13|7.14.0|chrome|120.0.0.0|||windows 10||zh-CN|||dW5kZWZpbmVk||";

const WEB_ORIGIN: HeaderValue = HeaderValue::from_static("https://yun.139.com");
const WEB_REFERER: HeaderValue = HeaderValue::from_static("https://yun.139.com/");
const WEB_APP_REFERER: HeaderValue = HeaderValue::from_static("https://yun.139.com/w/");

pub(super) fn build_refresh_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/xml;charset=UTF-8"),
    );
    headers.insert(REFERER, WEB_REFERER);
    headers
}

pub(super) fn build_upload_part_headers(size: u64) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    headers.insert(CONTENT_LENGTH, HeaderValue::from(size));
    headers.insert(ORIGIN, WEB_ORIGIN);
    headers.insert(REFERER, WEB_REFERER);
    headers
}

fn build_web_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();

    headers.insert(
        "Accept",
        HeaderValue::from_static("application/json, text/plain, */*"),
    );
    headers.insert("mcloud-channel", HeaderValue::from_static(MCLOUD_CHANNEL));
    headers.insert("mcloud-client", HeaderValue::from_static(MCLOUD_CLIENT));
    headers.insert("mcloud-version", HeaderValue::from_static(MCLOUD_VERSION));
    headers.insert(ORIGIN, WEB_ORIGIN);
    headers.insert(REFERER, WEB_APP_REFERER);
    headers.insert("x-DeviceInfo", HeaderValue::from_static(DEVICE_INFO));
    headers.insert(
        "x-huawei-channelSrc",
        HeaderValue::from_static(MCLOUD_CHANNEL_SRC),
    );
    headers.insert("x-inner-ntwk", HeaderValue::from_static("2"));
    headers.insert("x-m4c-caller", HeaderValue::from_static("PC"));
    headers.insert("x-m4c-src", HeaderValue::from_static("10002"));
    headers.insert("Inner-Hcy-Router-Https", HeaderValue::from_static("1"));

    headers
}

fn build_base_headers() -> HeaderMap {
    let mut headers = build_web_headers();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/json;charset=UTF-8"),
    );
    headers
}

pub fn build_route_headers(
    authorization: &str,
    ts: &str,
    rand_str: &str,
    sign: &str,
) -> Result<HeaderMap, ClientError> {
    let mut headers = build_base_headers();

    insert_signature(&mut headers, authorization, ts, rand_str, sign)?;
    headers.insert("x-SvcType", HeaderValue::from_static("1"));

    Ok(headers)
}

pub fn build_personal_headers(
    authorization: &str,
    ts: &str,
    rand_str: &str,
    sign: &str,
) -> Result<HeaderMap, ClientError> {
    let mut headers = build_web_headers();
    headers.insert("Caller", HeaderValue::from_static("web"));
    headers.insert("CMS-DEVICE", HeaderValue::from_static("default"));
    headers.insert("mcloud-route", HeaderValue::from_static("001"));
    headers.insert("x-yun-api-version", HeaderValue::from_static("v1"));
    headers.insert(
        "x-yun-app-channel",
        HeaderValue::from_static(MCLOUD_CHANNEL_SRC),
    );
    headers.insert(
        "x-yun-channel-source",
        HeaderValue::from_static(MCLOUD_CHANNEL_SRC),
    );
    headers.insert(
        "x-yun-client-info",
        HeaderValue::from_static(PC_CLIENT_INFO),
    );
    headers.insert("x-yun-module-type", HeaderValue::from_static("100"));

    headers.insert("x-yun-svc-type", HeaderValue::from_static("1"));
    headers.insert("x-SvcType", HeaderValue::from_static("1"));
    insert_signature(&mut headers, authorization, ts, rand_str, sign)?;
    Ok(headers)
}

fn insert_signature(
    headers: &mut HeaderMap,
    authorization: &str,
    ts: &str,
    rand_str: &str,
    sign: &str,
) -> Result<(), ClientError> {
    headers.insert(
        "Authorization",
        format!("Basic {}", authorization)
            .parse()
            .map_err(|e| ClientError::InvalidHeader(format!("{}", e)))?,
    );
    headers.insert(
        "mcloud-sign",
        format!("{},{},{}", ts, rand_str, sign)
            .parse()
            .map_err(|e| ClientError::InvalidHeader(format!("{}", e)))?,
    );
    Ok(())
}
