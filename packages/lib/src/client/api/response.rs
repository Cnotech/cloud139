use crate::{error::ClientError, models::common::BaseResp};
use serde_json::Value;

pub(super) fn is_response_successful(response: &Value) -> bool {
    response.get("success").and_then(Value::as_bool) == Some(true)
}

/// 批量接口可能在响应中为各条目分别返回成功标记。
pub(super) fn is_mutation_successful(response: &Value) -> bool {
    fn no_failure(value: &Value) -> bool {
        match value {
            Value::Object(map) => {
                map.get("success").and_then(Value::as_bool) != Some(false)
                    && map.values().all(no_failure)
            }
            Value::Array(items) => items.iter().all(no_failure),
            _ => true,
        }
    }

    is_response_successful(response) && no_failure(response)
}

pub(super) fn format_api_error(operation: &str, message: &str, code: Option<&str>) -> String {
    match code {
        Some(code) => format!("{operation}: {message}（错误码: {code}）"),
        None => format!("{operation}: {message}"),
    }
}

pub(super) fn json_api_error(operation: &str, response: &Value, fallback: &str) -> ClientError {
    ClientError::Api(format_api_error(
        operation,
        response["message"].as_str().unwrap_or(fallback),
        Some(response["code"].as_str().unwrap_or("未返回")),
    ))
}

pub(super) fn require_success(response: BaseResp, fallback: &str) -> Result<(), ClientError> {
    if !response.success {
        return Err(ClientError::Api(
            response.message.unwrap_or_else(|| fallback.to_owned()),
        ));
    }
    Ok(())
}
