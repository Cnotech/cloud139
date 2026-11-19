use serde_json::Value;

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
