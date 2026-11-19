use crate::error::ClientError;

#[derive(Clone, Copy)]
pub(crate) struct UploadLayout {
    file_size: i64,
    part_size: i64,
    part_count: i64,
}

impl UploadLayout {
    pub(crate) fn new(file_size: i64, part_size: i64) -> Result<Self, ClientError> {
        if file_size < 0 || part_size <= 0 {
            return Err(ClientError::Api(
                "文件大小必须非负，上传分片大小必须大于 0".into(),
            ));
        }
        let part_count = file_size / part_size + i64::from(file_size % part_size != 0);
        if part_count > i32::MAX as i64 {
            return Err(ClientError::Api("上传分片数量超出协议范围".into()));
        }
        Ok(Self {
            file_size,
            part_size,
            part_count,
        })
    }

    pub(crate) fn file_size(self) -> i64 {
        self.file_size
    }

    pub(crate) fn part_count(self) -> i64 {
        self.part_count
    }

    pub(crate) fn part_offset_and_size(self, index: i64) -> (i64, i64) {
        let offset = index * self.part_size;
        (offset, self.part_size.min(self.file_size - offset))
    }
}

#[cfg(test)]
mod tests {
    use super::UploadLayout;

    #[test]
    fn part_count_handles_large_sizes_and_rejects_invalid_layouts() {
        assert_eq!(
            UploadLayout::new(i64::MAX, i64::MAX).unwrap().part_count(),
            1
        );
        assert_eq!(UploadLayout::new(0, 100).unwrap().part_count(), 0);
        assert_eq!(UploadLayout::new(101, 100).unwrap().part_count(), 2);
        for (size, part) in [(1, 0), (1, -1), (-1, 100), (i64::MAX, 1)] {
            assert!(UploadLayout::new(size, part).is_err());
        }
    }
}
