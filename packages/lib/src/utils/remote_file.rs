use super::crypto::is_sha256_hex;
use crate::protocol;

pub(crate) fn is_non_root_file_id(id: &str) -> bool {
    !id.is_empty() && id != protocol::ROOT_FILE_ID
}

pub(crate) fn normalize_content_sha256(
    hash: Option<&str>,
    algorithm: Option<&str>,
) -> Option<String> {
    let hash = hash?.trim();
    let algorithm = algorithm?.trim();
    let is_sha256 = algorithm.eq_ignore_ascii_case(protocol::CONTENT_HASH_SHA256)
        || algorithm.eq_ignore_ascii_case(protocol::CONTENT_HASH_SHA256_ALIAS);
    (is_sha256 && is_sha256_hex(hash)).then(|| hash.to_ascii_lowercase())
}
