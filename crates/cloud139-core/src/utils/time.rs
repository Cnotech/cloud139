use chrono::NaiveDateTime;

/// 解析个人云时间字符串
pub fn parse_personal_time(time_str: &str) -> String {
    if time_str.is_empty() {
        return String::new();
    }
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(time_str) {
        return dt.format("%Y-%m-%d %H:%M:%S").to_string();
    }
    if let Ok(dt) = NaiveDateTime::parse_from_str(time_str, "%Y-%m-%dT%H:%M:%S%.f") {
        return dt.format("%Y-%m-%d %H:%M:%S").to_string();
    }
    time_str.to_string()
}
