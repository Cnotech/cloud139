//! Cloud path normalization and cross-platform name validation.
use crate::error::ClientError;

pub fn validate_name(name: &str) -> Result<(), ClientError> {
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.ends_with(['.', ' '])
        || name
            .chars()
            .any(|c| c.is_control() || "/\\:<>\"|?*".contains(c))
        || matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && matches!(stem.as_bytes()[3], b'1'..=b'9'))
    {
        return Err(crate::error::invalid(format!(
            "非法或不安全的名称: {name:?}"
        )));
    }
    Ok(())
}

pub fn normalize(path: &str) -> Result<String, ClientError> {
    let mut parts = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            _ => {
                validate_name(part)?;
                parts.push(part);
            }
        }
    }
    Ok(format!("/{}", parts.join("/")))
}

pub fn non_root(path: &str) -> Result<String, ClientError> {
    let path = normalize(path)?;
    if path == "/" {
        return Err(ClientError::CannotOperateOnRoot);
    }
    Ok(path)
}

pub fn normalize_cloud_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() || trimmed == "/" {
        return "/".to_string();
    }
    format!("/{}", trimmed.trim_start_matches('/').trim_end_matches('/'))
}

pub fn cloud_child_path(parent: &str, name: &str) -> String {
    format!(
        "{}/{}",
        parent.trim_end_matches('/'),
        name.trim_matches('/')
    )
}
