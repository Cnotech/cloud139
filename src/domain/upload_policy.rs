/// Conservative limit for the existing personal-cloud upload API.
/// Exactly 5 GiB is allowed; family/group use different upload APIs.
pub const PERSONAL_UPLOAD_MAX_BYTES: u64 = 5 * 1024 * 1024 * 1024;
