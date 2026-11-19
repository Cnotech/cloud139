mod personal;
mod personal_parts;

use super::sync_service::CloudSyncScan;

pub(super) use personal::{upload_sync_to_parent, upload_to_parent};

pub(super) struct SyncUploadPlan<'a> {
    pub(super) host: &'a str,
    pub(super) scan: Option<&'a CloudSyncScan>,
    pub(super) rel_path: &'a str,
    pub(super) parent_id: &'a str,
    pub(super) remote_parent: &'a str,
    pub(super) file_name: &'a str,
    pub(super) replace_directory_id: Option<&'a str>,
    pub(super) hash_missing: Option<&'a mut bool>,
    pub(super) file_changed: &'a mut bool,
}
