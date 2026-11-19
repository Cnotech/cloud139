use crate::{client::ClientError, models::BaseResp, protocol};
use serde_json::Value;

pub(crate) fn format_api_error(operation: &str, message: &str, code: Option<&str>) -> String {
    match code {
        Some(code) => format!("{operation}: {message}（错误码: {code}）"),
        None => format!("{operation}: {message}"),
    }
}

pub(crate) fn upload_creation_error(response: &BaseResp, file_size: i64) -> ClientError {
    let code = response.code.as_deref();
    let message = format_api_error(
        "创建上传任务失败",
        response.message.as_deref().unwrap_or("未知错误"),
        code,
    );
    let message = if code == Some(protocol::UPLOAD_ENTITLEMENT_ERROR_CODE) {
        format!("{message}；文件大小 {file_size} 字节，请核对账号当前的单文件上传额度")
    } else {
        message
    };
    ClientError::Api(message)
}

pub(crate) fn json_api_error(operation: &str, response: &Value, fallback: &str) -> ClientError {
    ClientError::Api(format_api_error(
        operation,
        response["message"].as_str().unwrap_or(fallback),
        Some(response["code"].as_str().unwrap_or("未返回")),
    ))
}

pub(crate) fn require_success(response: BaseResp, fallback: &str) -> Result<(), ClientError> {
    if !response.success {
        return Err(ClientError::Api(
            response.message.unwrap_or_else(|| fallback.to_owned()),
        ));
    }
    Ok(())
}

pub(crate) fn is_response_successful(response: &Value) -> bool {
    response.get("success").and_then(Value::as_bool) == Some(true)
}

/// 批量接口可能在响应中为各条目分别返回成功标记。
pub(crate) fn is_mutation_successful(response: &Value) -> bool {
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
