use crate::client::{
    CLIENT_INFO, ClientError, DEVICE_INFO, MCLOUD_CHANNEL, MCLOUD_CHANNEL_SRC, MCLOUD_CLIENT,
    MCLOUD_VERSION,
};
use reqwest::header::{HeaderMap, HeaderValue};

fn build_base_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();

    headers.insert(
        "Accept",
        HeaderValue::from_static("application/json, text/plain, */*"),
    );
    headers.insert(
        "Content-Type",
        HeaderValue::from_static("application/json;charset=UTF-8"),
    );
    headers.insert("mcloud-channel", HeaderValue::from_static(MCLOUD_CHANNEL));
    headers.insert("mcloud-client", HeaderValue::from_static(MCLOUD_CLIENT));
    headers.insert("mcloud-version", HeaderValue::from_static(MCLOUD_VERSION));
    headers.insert("Origin", HeaderValue::from_static("https://yun.139.com"));
    headers.insert(
        "Referer",
        HeaderValue::from_static("https://yun.139.com/w/"),
    );
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

pub fn build_route_headers(
    authorization: &str,
    ts: &str,
    rand_str: &str,
    sign: &str,
) -> Result<HeaderMap, ClientError> {
    let mut headers = build_base_headers();

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
    headers.insert("x-SvcType", HeaderValue::from_static("1"));

    Ok(headers)
}

pub fn build_common_headers() -> HeaderMap {
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
    headers.insert(
        "Referer",
        HeaderValue::from_static("https://yun.139.com/w/"),
    );
    headers.insert("x-DeviceInfo", HeaderValue::from_static(DEVICE_INFO));
    headers.insert(
        "x-huawei-channelSrc",
        HeaderValue::from_static(MCLOUD_CHANNEL_SRC),
    );
    headers.insert("x-inner-ntwk", HeaderValue::from_static("2"));
    headers.insert("x-m4c-caller", HeaderValue::from_static("PC"));
    headers.insert("x-m4c-src", HeaderValue::from_static("10002"));
    headers.insert("x-yun-api-version", HeaderValue::from_static("v1"));
    headers.insert(
        "x-yun-app-channel",
        HeaderValue::from_static(MCLOUD_CHANNEL_SRC),
    );
    headers.insert(
        "x-yun-channel-source",
        HeaderValue::from_static(MCLOUD_CHANNEL_SRC),
    );
    headers.insert("x-yun-client-info", HeaderValue::from_static(CLIENT_INFO));
    headers.insert("x-yun-module-type", HeaderValue::from_static("100"));
    headers.insert("Inner-Hcy-Router-Https", HeaderValue::from_static("1"));

    headers
}

pub fn build_signed_headers(
    authorization: &str,
    ts: &str,
    rand_str: &str,
    sign: &str,
    svc_type: &str,
) -> Result<HeaderMap, ClientError> {
    let mut headers = build_common_headers();

    headers.insert(
        "x-yun-svc-type",
        HeaderValue::from_str(svc_type)
            .map_err(|e| ClientError::InvalidHeader(format!("{}", e)))?,
    );

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
    headers.insert(
        "x-SvcType",
        HeaderValue::from_str(svc_type)
            .map_err(|e| ClientError::InvalidHeader(format!("{}", e)))?,
    );

    Ok(headers)
}

pub fn build_group_signed_headers(
    authorization: &str,
    ts: &str,
    rand_str: &str,
    sign: &str,
    svc_type: &str,
) -> Result<HeaderMap, ClientError> {
    let mut headers = build_base_headers();

    headers.insert(
        "x-SvcType",
        HeaderValue::from_str(svc_type)
            .map_err(|e| ClientError::InvalidHeader(format!("{}", e)))?,
    );
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

    Ok(headers)
}
