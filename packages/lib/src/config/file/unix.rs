use std::fs::{File, Permissions};
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use tempfile::NamedTempFile;

pub(super) fn restrict_permissions(file: &File) -> io::Result<()> {
    file.set_permissions(Permissions::from_mode(0o600))
}

pub(super) fn persist(
    temp: NamedTempFile,
    path: &Path,
    parent: &Path,
    overwrite: bool,
) -> io::Result<()> {
    if overwrite {
        temp.persist(path).map_err(|err| err.error)?;
    } else {
        temp.persist_noclobber(path).map_err(|err| err.error)?;
    }
    // 同步目录，保证重命名的目录项也被持久化。
    File::open(parent)?.sync_all()
}
