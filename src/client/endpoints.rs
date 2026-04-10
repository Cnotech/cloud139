pub const ROUTE_POLICY_URL: &str = "https://user-njs.yun.139.com/user/route/qryRoutePolicy";

pub const AUTH_TOKEN_REFRESH_URL: &str =
    "https://aas.caiyun.feixin.10086.cn/tellin/authTokenRefresh.do";

pub mod family {
    pub const ALBUM_BASE_URL: &str =
        "https://group.yun.139.com/hcy/family/adapter/andAlbum/openApi";

    pub mod orchestration {
        pub const GET_FILE_UPLOAD_URL: &str =
            "https://yun.139.com/orchestration/familyCloud-rebuild/content/v1.0/getFileUploadURL";
        pub const GET_FILE_DOWNLOAD_URL: &str =
            "https://yun.139.com/orchestration/familyCloud-rebuild/content/v1.0/getFileDownLoadURL";
        pub const QUERY_CONTENT_LIST: &str =
            "https://yun.139.com/orchestration/familyCloud-rebuild/content/v1.2/queryContentList";
        pub const MODIFY_CONTENT_INFO: &str =
            "https://yun.139.com/orchestration/familyCloud-rebuild/photoContent/v1.0/modifyContentInfo";
        pub const CREATE_BATCH_OPR_TASK: &str =
            "https://yun.139.com/orchestration/familyCloud-rebuild/batchOprTask/v1.0/createBatchOprTask";
        pub const CREATE_CLOUD_DOC: &str =
            "https://yun.139.com/orchestration/familyCloud-rebuild/cloudCatalog/v1.0/createCloudDoc";
    }
}

pub mod group {
    pub const MUTUAL_BASE_URL: &str = "https://group.yun.139.com/hcy/mutual/adapter";

    pub mod orchestration {
        pub const GET_FILE_UPLOAD_URL: &str =
            "https://yun.139.com/orchestration/group-rebuild/content/v1.0/getGroupFileUploadURL";
        pub const GET_FILE_DOWNLOAD_URL: &str =
            "https://yun.139.com/orchestration/group-rebuild/groupManage/v1.0/getGroupFileDownLoadURL";
        pub const QUERY_GROUP_CONTENT_LIST: &str =
            "https://yun.139.com/orchestration/group-rebuild/content/v1.0/queryGroupContentList";
        pub const QUERY_GROUP_CATALOG: &str =
            "https://yun.139.com/orchestration/group-rebuild/catalog/v1.0/queryGroupContentList";
        pub const MODIFY_GROUP_CATALOG: &str =
            "https://yun.139.com/orchestration/group-rebuild/catalog/v1.0/modifyGroupCatalog";
        pub const MODIFY_GROUP_CONTENT: &str =
            "https://yun.139.com/orchestration/group-rebuild/content/v1.0/modifyGroupContent";
        pub const CREATE_BATCH_OPR_TASK: &str =
            "https://yun.139.com/orchestration/group-rebuild/task/v1.0/createBatchOprTask";
        pub const CREATE_GROUP_CATALOG: &str =
            "https://yun.139.com/orchestration/group-rebuild/catalog/v1.0/createGroupCatalog";
    }
}
