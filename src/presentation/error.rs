use crate::client::ClientError;

pub fn format_error(e: &ClientError) -> String {
    match e {
        ClientError::ForceRequired => "请使用 --force 参数确认继续".to_string(),
        ClientError::ConfirmationRequired => "请使用 --yes 参数确认删除".to_string(),
        ClientError::InvalidSourcePath => "无效的源文件路径".to_string(),
        ClientError::FileNotFound => "文件不存在".to_string(),
        ClientError::CannotOperateOnRoot => "不能操作根目录".to_string(),
        ClientError::NoSourceFiles => "没有有效的源文件需要处理".to_string(),
        ClientError::UnsupportedFamilyBatchMove => "家庭云暂不支持批量移动".to_string(),
        ClientError::UnsupportedGroupBatchMove => "群组云暂不支持批量移动".to_string(),
        ClientError::UnsupportedFamilyRenameFolder => "家庭云不支持重命名文件夹".to_string(),
        ClientError::UnsupportedDownloadDirectory => {
            "不支持下载目录，请使用 ls 命令查看目录内容".to_string()
        }
        ClientError::InvalidFilePath => "无效的文件路径".to_string(),
        ClientError::OperationCancelled => "操作被取消".to_string(),
        _ => e.to_string(),
    }
}
