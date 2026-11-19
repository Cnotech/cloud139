use crate::client::ClientError;
use crate::protocol::{BASIC_AUTH_PREFIX, SIGNATURE_HEADER, WEB_HEADERS};
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};

pub(in crate::client) fn insert_static_headers(
    headers: &mut HeaderMap,
    values: &[(&'static str, &'static str)],
) {
    for &(name, value) in values {
        headers.insert(name, HeaderValue::from_static(value));
    }
}

pub(in crate::client) fn build_signed_headers(
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
