//! 个人云与路由请求的签名请求头及客户端标识。

use crate::client::http::insert_static_headers;
use crate::error::ClientError;
use crate::protocol::{BASIC_AUTH_PREFIX, SIGNATURE_HEADER, WEB_ORIGIN};
use reqwest::header::{AUTHORIZATION, HeaderMap};

pub const MCLOUD_VERSION: &str = "7.14.0";
pub const MCLOUD_CLIENT: &str = "10701";
pub const MCLOUD_CHANNEL: &str = "1000101";
pub const MCLOUD_CHANNEL_SRC: &str = "10000034";
pub const DEVICE_INFO: &str = "||9|7.14.0|chrome|120.0.0.0|||windows 10||zh-CN|||";
pub const PC_CLIENT_INFO: &str =
    "||13|7.14.0|chrome|120.0.0.0|||windows 10||zh-CN|||dW5kZWZpbmVk||";

const WEB_HEADERS: &[(&str, &str)] = &[
    ("accept", "application/json, text/plain, */*"),
    ("mcloud-channel", MCLOUD_CHANNEL),
    ("mcloud-client", MCLOUD_CLIENT),
    ("mcloud-version", MCLOUD_VERSION),
    ("origin", WEB_ORIGIN),
    ("referer", "https://yun.139.com/w/"),
    ("x-deviceinfo", DEVICE_INFO),
    ("x-huawei-channelsrc", MCLOUD_CHANNEL_SRC),
    ("x-inner-ntwk", "2"),
    ("x-m4c-caller", "PC"),
    ("x-m4c-src", "10002"),
    ("inner-hcy-router-https", "1"),
];
const ROUTE_HEADERS: &[(&str, &str)] = &[
    ("content-type", "application/json;charset=UTF-8"),
    ("x-svctype", "1"),
];
const PERSONAL_HEADERS: &[(&str, &str)] = &[
    ("caller", "web"),
    ("cms-device", "default"),
    ("mcloud-route", "001"),
    ("x-yun-api-version", "v1"),
    ("x-yun-app-channel", MCLOUD_CHANNEL_SRC),
    ("x-yun-channel-source", MCLOUD_CHANNEL_SRC),
    ("x-yun-client-info", PC_CLIENT_INFO),
    ("x-yun-module-type", "100"),
    ("x-yun-svc-type", "1"),
    ("x-svctype", "1"),
];

pub fn build_route_headers(
    authorization: &str,
    ts: &str,
    rand_str: &str,
    sign: &str,
) -> Result<HeaderMap, ClientError> {
    build_signed_headers(ROUTE_HEADERS, authorization, ts, rand_str, sign)
}

pub fn build_personal_headers(
    authorization: &str,
    ts: &str,
    rand_str: &str,
    sign: &str,
) -> Result<HeaderMap, ClientError> {
    build_signed_headers(PERSONAL_HEADERS, authorization, ts, rand_str, sign)
}

fn build_signed_headers(
    extra: &[(&'static str, &'static str)],
    authorization: &str,
    ts: &str,
    nonce: &str,
    sign: &str,
) -> Result<HeaderMap, ClientError> {
    let mut headers = HeaderMap::new();
    insert_static_headers(&mut headers, WEB_HEADERS);
    insert_static_headers(&mut headers, extra);
    insert_signature(&mut headers, authorization, ts, nonce, sign)?;
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
        AUTHORIZATION,
        format!("{BASIC_AUTH_PREFIX}{authorization}")
            .parse()
            .map_err(|e| ClientError::InvalidHeader(format!("{}", e)))?,
    );
    headers.insert(
        SIGNATURE_HEADER,
        format!("{},{},{}", ts, rand_str, sign)
            .parse()
            .map_err(|e| ClientError::InvalidHeader(format!("{}", e)))?,
    );
    Ok(())
}
