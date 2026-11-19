//! 139 云盘接口的固定值和标识规则，供请求构造和响应解析共用。

// API 地址与端点。
pub(crate) const ROUTE_POLICY_URL: &str = "https://user-njs.yun.139.com/user/route/qryRoutePolicy";
pub(crate) const AUTH_TOKEN_REFRESH_URL: &str =
    "https://aas.caiyun.feixin.10086.cn/tellin/authTokenRefresh.do";
pub(crate) const LIST_FILES: &str = "/file/list";
pub(crate) const QUERY_FILE: &str = "/file/query";
pub(crate) const CREATE_FILE: &str = "/file/create";
pub(crate) const UPDATE_FILE: &str = "/file/update";
pub(crate) const BATCH_MOVE: &str = "/file/batchMove";
pub(crate) const BATCH_COPY: &str = "/file/batchCopy";
pub(crate) const BATCH_TRASH: &str = "/recyclebin/batchTrash";
pub(crate) const DOWNLOAD_URL: &str = "/file/getDownloadUrl";
pub(crate) const UPLOAD_URL: &str = "/file/getUploadUrl";
pub(crate) const COMPLETE_UPLOAD: &str = "/file/complete";

// 业务返回码。
pub(crate) const UPLOAD_ENTITLEMENT_ERROR_CODE: &str = "04010319";
pub(crate) const TOKEN_REFRESH_SUCCESS_CODE: &str = "0";

// 路由、签名与续期标识。
pub(crate) const PERSONAL_MODULE: &str = "personal";
pub(crate) const ROUTE_USER_TYPE: i32 = 1;
pub(crate) const ROUTE_ADDRESS_TYPE: i32 = 1;
pub(crate) const PERSONAL_ACCOUNT_TYPE: i32 = 1;
pub(crate) const BASIC_AUTH_PREFIX: &str = "Basic ";
pub(crate) const PC_TOKEN_PREFIX: &str = "pc";
pub(crate) const TOKEN_REFRESH_CLIENT_TYPE: u16 = 656;
pub(crate) const TOKEN_REFRESH_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36";
pub(crate) const SIGNATURE_HEADER: &str = "mcloud-sign";
pub(crate) const SIGNATURE_TIME_FORMAT: &str = "%Y-%m-%d %H:%M:%S";
pub(crate) const SIGNATURE_NONCE_LENGTH: usize = 16;

// 文件标识、列表与上传参数。
pub(crate) const ROOT_FILE_ID: &str = "/";
pub(crate) const FILE_TYPE_FILE: &str = "file";
pub(crate) const FILE_TYPE_FOLDER: &str = "folder";
pub(crate) const FILE_TYPE_DIR: &str = "dir";
pub(crate) const FILE_TYPE_DIRECTORY_CODE: &str = "1";
pub(crate) const LIST_ORDER_FIELD: &str = "updated_at";
pub(crate) const LIST_ORDER_DIRECTION: &str = "DESC";
pub(crate) const STANDARD_THUMBNAILS: &[&str] = &["Small", "Large"];
pub(crate) const THUMBNAILS_WITH_ORIGINAL: &[&str] = &["Small", "Large", "Original"];
pub(crate) const CONTENT_HASH_SHA256: &str = "SHA256";
pub(crate) const CONTENT_HASH_SHA256_ALIAS: &str = "SHA-256";
pub(crate) const UPLOAD_AUTO_RENAME: &str = "auto_rename";
pub(crate) const UPLOAD_PARTS_PER_REQUEST: i64 = 100;

pub(crate) fn is_non_root_file_id(id: &str) -> bool {
    !id.is_empty() && id != ROOT_FILE_ID
}

// HTTP 请求头的固定值。
pub(crate) const WEB_ORIGIN: &str = "https://yun.139.com";
const WEB_REFERER: &str = "https://yun.139.com/";
pub(crate) const JSON_CONTENT_TYPE: &str = "application/json";
pub(crate) const TOKEN_REFRESH_HEADERS: &[(&str, &str)] = &[
    ("content-type", "application/xml;charset=UTF-8"),
    ("referer", WEB_REFERER),
];
pub(crate) const UPLOAD_PART_HEADERS: &[(&str, &str)] = &[
    ("content-type", "application/octet-stream"),
    ("origin", WEB_ORIGIN),
    ("referer", WEB_REFERER),
];
