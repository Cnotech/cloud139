use crate::client::ClientError;

pub fn get_part_size(size: i64, custom_size: i64) -> i64 {
    if custom_size != 0 {
        return custom_size;
    }
    if size / (1024 * 1024 * 1024) > 30 {
        return 512 * 1024 * 1024;
    }
    100 * 1024 * 1024
}

pub(crate) fn part_offset_and_size(index: i64, part_size: i64, file_size: i64) -> (i64, i64) {
    let offset = index * part_size;
    (offset, part_size.min(file_size - offset))
}

pub(crate) fn checked_part_count(file_size: i64, part_size: i64) -> Result<i64, ClientError> {
    if file_size < 0 || part_size <= 0 {
        return Err(ClientError::Api(
            "文件大小必须非负，上传分片大小必须大于 0".into(),
        ));
    }
    let count = file_size / part_size + i64::from(file_size % part_size != 0);
    if count > i32::MAX as i64 {
        return Err(ClientError::Api("上传分片数量超出协议范围".into()));
    }
    Ok(count)
}
