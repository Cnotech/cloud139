use crate::client::ClientError;
use crate::protocol::{
    BASIC_AUTH_PREFIX, PERSONAL_HEADERS, ROUTE_HEADERS, SIGNATURE_HEADER, TOKEN_REFRESH_HEADERS,
    UPLOAD_PART_HEADERS, WEB_HEADERS,
};
pub use crate::protocol::{
    DEVICE_INFO, MCLOUD_CHANNEL, MCLOUD_CHANNEL_SRC, MCLOUD_CLIENT, MCLOUD_VERSION, PC_CLIENT_INFO,
};
use reqwest::header::{AUTHORIZATION, CONTENT_LENGTH, HeaderMap, HeaderValue};

fn insert_static_headers(headers: &mut HeaderMap, values: &[(&'static str, &'static str)]) {
    for &(name, value) in values {
        headers.insert(name, HeaderValue::from_static(value));
    }
}

pub(super) fn build_refresh_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    insert_static_headers(&mut headers, TOKEN_REFRESH_HEADERS);
    headers
}

pub(super) fn build_upload_part_headers(size: u64) -> HeaderMap {
    let mut headers = HeaderMap::new();
    insert_static_headers(&mut headers, UPLOAD_PART_HEADERS);
    headers.insert(CONTENT_LENGTH, HeaderValue::from(size));
    headers
}

pub fn build_route_headers(
    authorization: &str,
    ts: &str,
    rand_str: &str,
    sign: &str,
) -> Result<HeaderMap, ClientError> {
    let mut headers = HeaderMap::new();
    insert_static_headers(&mut headers, WEB_HEADERS);
    insert_static_headers(&mut headers, ROUTE_HEADERS);
    insert_signature(&mut headers, authorization, ts, rand_str, sign)?;
    Ok(headers)
}

pub fn build_personal_headers(
    authorization: &str,
    ts: &str,
    rand_str: &str,
    sign: &str,
) -> Result<HeaderMap, ClientError> {
    let mut headers = HeaderMap::new();
    insert_static_headers(&mut headers, WEB_HEADERS);
    insert_static_headers(&mut headers, PERSONAL_HEADERS);
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
