use std::path::Path;

/// 根据远程路径和可选的本地路径，解析出最终的本地保存路径。
pub fn resolve_local_path(remote_path: &str, local_path: &Option<String>) -> String {
    match local_path {
        Some(path) if !path.is_empty() => {
            let ends_with_slash = path.ends_with('/');
            let path = path.trim_end_matches('/');
            let path_obj = Path::new(path);
            if path_obj.is_dir() || ends_with_slash {
                let parts: Vec<&str> = remote_path.trim_start_matches('/').rsplit('/').collect();
                let file_name = parts.first().copied().unwrap_or(remote_path);
                if file_name.is_empty() || file_name == remote_path {
                    format!("{}/download", path)
                } else {
                    format!("{}/{}", path, file_name)
                }
            } else {
                path.to_string()
            }
        }
        _ => {
            let parts: Vec<&str> = remote_path.trim_start_matches('/').rsplit('/').collect();
            let file_name = parts.first().copied().unwrap_or(remote_path);
            if file_name.is_empty() || file_name == remote_path {
                "download".to_string()
            } else {
                file_name.to_string()
            }
        }
    }
}
