#![allow(dead_code)]

pub mod mock_api_client {
    use async_trait::async_trait;
    use cloud139::client::api_trait::ApiClient;
    use cloud139::client::ClientError;
    use cloud139::domain::StorageType;
    use cloud139::config::Config;
    use cloud139::models::*;
    use mockall::mock;
    use std::collections::HashMap;
    use std::sync::Mutex;

    mock! {
        pub ApiClientMock {
            fn get_personal_cloud_host(&self, config: &mut Config) -> impl std::future::Future<Output = Result<String, ClientError>> + Send;
            fn personal_api_request<T: for<'de> serde::Deserialize<'de> + Send>(&self, config: &Config, url: &str, body: serde_json::Value, storage_type: StorageType) -> impl std::future::Future<Output = Result<T, ClientError>> + Send;
            fn get_file_id_by_path(&self, config: &Config, path: &str) -> impl std::future::Future<Output = Result<String, ClientError>> + Send;
            fn check_file_exists(&self, config: &Config, parent_file_id: &str, file_name: &str) -> impl std::future::Future<Output = Result<bool, ClientError>> + Send;
            fn list_personal_files(&self, config: &Config, parent_file_id: &str) -> impl std::future::Future<Output = Result<Vec<PersonalFileItem>, ClientError>> + Send;
            fn get_personal_download_link(&self, config: &Config, file_id: &str) -> impl std::future::Future<Output = Result<String, ClientError>> + Send;
            fn get_family_download_link(&self, config: &Config, content_id: &str, path: &str) -> impl std::future::Future<Output = Result<String, ClientError>> + Send;
            fn get_group_download_link(&self, config: &Config, content_id: &str, path: &str) -> impl std::future::Future<Output = Result<String, ClientError>> + Send;
        }
    }

    #[async_trait]
    impl ApiClient for ApiClientMock {
        async fn get_personal_cloud_host(&self, config: &mut Config) -> Result<String, ClientError> {
            self.get_personal_cloud_host(config).await
        }

        async fn personal_api_request<T: for<'de> serde::Deserialize<'de> + Send>(
            &self, config: &Config, url: &str, body: serde_json::Value, storage_type: StorageType
        ) -> Result<T, ClientError> {
            self.personal_api_request(config, url, body, storage_type).await
        }

        async fn get_file_id_by_path(&self, config: &Config, path: &str) -> Result<String, ClientError> {
            self.get_file_id_by_path(config, path).await
        }

        async fn check_file_exists(&self, config: &Config, parent_file_id: &str, file_name: &str) -> Result<bool, ClientError> {
            self.check_file_exists(config, parent_file_id, file_name).await
        }

        async fn list_personal_files(&self, config: &Config, parent_file_id: &str) -> Result<Vec<PersonalFileItem>, ClientError> {
            self.list_personal_files(config, parent_file_id).await
        }

        async fn get_personal_download_link(&self, config: &Config, file_id: &str) -> Result<String, ClientError> {
            self.get_personal_download_link(config, file_id).await
        }

        async fn get_family_download_link(&self, config: &Config, content_id: &str, path: &str) -> Result<String, ClientError> {
            self.get_family_download_link(config, content_id, path).await
        }

        async fn get_group_download_link(&self, config: &Config, content_id: &str, path: &str) -> Result<String, ClientError> {
            self.get_group_download_link(config, content_id, path).await
        }
    }

    pub struct MockApiClient {
        pub host: String,
        pub files: HashMap<String, Vec<PersonalFileItem>>,
        pub download_urls: HashMap<String, String>,
    }

    impl MockApiClient {
        pub fn new() -> Self {
            let mut files = HashMap::new();
            files.insert("/".to_string(), vec![
                PersonalFileItem {
                    file_id: Some("folder_1".to_string()),
                    name: Some("test_folder".to_string()),
                    size: None,
                    file_type: Some("folder".to_string()),
                    created_at: None,
                    updated_at: Some("2024-01-01T00:00:00Z".to_string()),
                    create_date: None,
                    update_date: None,
                    last_modified: None,
                    thumbnail_urls: None,
                },
                PersonalFileItem {
                    file_id: Some("file_1".to_string()),
                    name: Some("test_file.txt".to_string()),
                    size: Some(1024),
                    file_type: Some("file".to_string()),
                    created_at: None,
                    updated_at: Some("2024-01-02T00:00:00Z".to_string()),
                    create_date: None,
                    update_date: None,
                    last_modified: None,
                    thumbnail_urls: None,
                },
            ]);
            
            let mut download_urls = HashMap::new();
            download_urls.insert("file_1".to_string(), "https://download.example.com/file_1".to_string());

            Self {
                host: "https://personal.cloud.139.com".to_string(),
                files,
                download_urls,
            }
        }

        pub fn with_host(mut self, host: &str) -> Self {
            self.host = host.to_string();
            self
        }

        pub fn with_file(mut self, parent_id: &str, item: PersonalFileItem) -> Self {
            self.files.entry(parent_id.to_string()).or_insert_with(Vec::new).push(item);
            self
        }
    }

    impl Default for MockApiClient {
        fn default() -> Self {
            Self::new()
        }
    }

    #[async_trait]
    impl ApiClient for MockApiClient {
        async fn get_personal_cloud_host(&self, config: &mut Config) -> Result<String, ClientError> {
            if config.personal_cloud_host.is_some() {
                return Ok(config.personal_cloud_host.clone().unwrap());
            }
            Ok(self.host.clone())
        }

        async fn personal_api_request<T: for<'de> serde::Deserialize<'de> + Send>(
            &self, _config: &Config, url: &str, _body: serde_json::Value, _storage_type: StorageType
        ) -> Result<T, ClientError> {
            Err(ClientError::Other(format!("Not implemented: {}", url)))
        }

        async fn get_file_id_by_path(&self, config: &Config, path: &str) -> Result<String, ClientError> {
            if path.is_empty() || path == "/" {
                return Ok(String::new());
            }

            let host = self.host.clone();
            let parts: Vec<&str> = path.trim_start_matches('/').split('/').filter(|s| !s.is_empty()).collect();
            let mut current_id = String::new();

            for (i, part) in parts.iter().enumerate() {
                let parent_id = if current_id.is_empty() { "/".to_string() } else { current_id.clone() };
                
                let items = self.files.get(&parent_id).cloned().unwrap_or_default();
                let target = items.iter().find(|item| item.name.as_deref() == Some(*part));
                
                match target {
                    Some(item) => {
                        if i == parts.len() - 1 {
                            return Ok(item.file_id.clone().unwrap_or_default());
                        }
                        current_id = item.file_id.clone().unwrap_or_default();
                    }
                    None => {
                        return Err(ClientError::Api(format!("File not found: {}", part)));
                    }
                }
            }

            Ok(current_id)
        }

        async fn check_file_exists(&self, _config: &Config, parent_file_id: &str, file_name: &str) -> Result<bool, ClientError> {
            let items = self.files.get(parent_file_id).cloned().unwrap_or_default();
            Ok(items.iter().any(|item| item.name.as_deref() == Some(file_name)))
        }

        async fn list_personal_files(&self, _config: &Config, parent_file_id: &str) -> Result<Vec<PersonalFileItem>, ClientError> {
            Ok(self.files.get(parent_file_id).cloned().unwrap_or_default())
        }

        async fn get_personal_download_link(&self, _config: &Config, file_id: &str) -> Result<String, ClientError> {
            self.download_urls.get(file_id)
                .cloned()
                .ok_or_else(|| ClientError::Other("Download URL not found".to_string()))
        }

        async fn get_family_download_link(&self, _config: &Config, _content_id: &str, _path: &str) -> Result<String, ClientError> {
            Ok("https://family.download.example.com".to_string())
        }

        async fn get_group_download_link(&self, _config: &Config, _content_id: &str, _path: &str) -> Result<String, ClientError> {
            Ok("https://group.download.example.com".to_string())
        }
    }

    pub struct TestConfigBuilder {
        config: Config,
    }

    impl TestConfigBuilder {
        pub fn new() -> Self {
            Self {
                config: Config::default(),
            }
        }

        pub fn with_account(mut self, account: &str) -> Self {
            self.config.account = account.to_string();
            self
        }

        pub fn with_authorization(mut self, auth: &str) -> Self {
            self.config.authorization = auth.to_string();
            self
        }

        pub fn with_cloud_id(mut self, cloud_id: &str) -> Self {
            self.config.cloud_id = Some(cloud_id.to_string());
            self
        }

        pub fn with_storage_type(mut self, storage_type: &str) -> Self {
            self.config.storage_type = storage_type.to_string();
            self
        }

        pub fn with_personal_cloud_host(mut self, host: &str) -> Self {
            self.config.personal_cloud_host = Some(host.to_string());
            self
        }

        pub fn with_token_expired(mut self, expired: bool) -> Self {
            if expired {
                let past = chrono::Utc::now().timestamp_millis() - 24 * 60 * 60 * 1000;
                self.config.token_expire_time = Some(past);
            } else {
                let future = chrono::Utc::now().timestamp_millis() + 24 * 60 * 60 * 1000 * 30;
                self.config.token_expire_time = Some(future);
            }
            self
        }

        pub fn build(self) -> Config {
            self.config
        }
    }

    impl Default for TestConfigBuilder {
        fn default() -> Self {
            Self::new()
                .with_account("test@139.com")
                .with_authorization("Basic dGVzdA==")
                .with_storage_type("personal_new")
        }
    }
}

pub mod http_mock_helpers {
    use httpmock::prelude::*;
    use serde_json::json;

    pub struct MockServer {
        server: httpmock::MockServer,
    }

    impl MockServer {
        pub fn start() -> Self {
            let server = httpmock::MockServer::start();
            Self { server }
        }

        pub fn url(&self) -> &str {
            self.server.url("/").as_str()
        }

        pub fn mock_route_policy(&self, host: &str) -> httpmock::Mock {
            self.server.mock(|when, then| {
                when.method(POST)
                    .path("/user/route/qryRoutePolicy")
                    .header("Content-Type", "application/json;charset=UTF-8");
                then.status(200).json_body(json!({
                    "code": "0",
                    "success": true,
                    "data": {
                        "route_policy_list": [
                            {
                                "mod_name": "personal",
                                "https_url": host
                            }
                        ]
                    }
                }));
            })
        }

        pub fn mock_list_files(&self, parent_id: &str, files: Vec<serde_json::Value>) -> httpmock::Mock {
            self.server.mock(|when, then| {
                when.method(POST)
                    .path("/file/list")
                    .header("Content-Type", "application/json;charset=UTF-8")
                    .json_body(json!({
                        "parentFileId": parent_id
                    }));
                then.status(200).json_body(json!({
                    "success": true,
                    "data": {
                        "items": files,
                        "nextPageCursor": ""
                    }
                }));
            })
        }

        pub fn mock_download_url(&self, file_id: &str, url: &str) -> httpmock::Mock {
            self.server.mock(|when, then| {
                when.method(POST)
                    .path("/file/getDownloadUrl")
                    .header("Content-Type", "application/json;charset=UTF-8")
                    .json_body(json!({
                        "fileId": file_id
                    }));
                then.status(200).json_body(json!({
                    "success": true,
                    "data": {
                        "url": url,
                        "cdnUrl": "",
                        "fileName": "test.txt"
                    }
                }));
            })
        }

        pub fn mock_mkdir(&self, folder_name: &str, file_id: &str) -> httpmock::Mock {
            self.server.mock(|when, then| {
                when.method(POST)
                    .path("/file/createFolder")
                    .header("Content-Type", "application/json;charset=UTF-8");
                then.status(200).json_body(json!({
                    "success": true,
                    "message": "文件夹创建成功",
                    "data": {
                        "fileId": file_id,
                        "name": folder_name
                    }
                }));
            })
        }

        pub fn mock_delete(&self) -> httpmock::Mock {
            self.server.mock(|when, then| {
                when.method(POST)
                    .path("/recyclebin/batchTrash");
                then.status(200).json_body(json!({
                    "success": true,
                    "message": "文件已移动到回收站"
                }));
            })
        }

        pub fn mock_rename(&self) -> httpmock::Mock {
            self.server.mock(|when, then| {
                when.method(POST)
                    .path("/file/batchRename");
                then.status(200).json_body(json!({
                    "success": true,
                    "message": "重命名成功"
                }));
            })
        }

        pub fn mock_move(&self) -> httpmock::Mock {
            self.server.mock(|when, then| {
                when.method(POST)
                    .path("/file/batchMove");
                then.status(200).json_body(json!({
                    "success": true,
                    "message": "移动成功"
                }));
            })
        }

        pub fn mock_copy(&self) -> httpmock::Mock {
            self.server.mock(|when, then| {
                when.method(POST)
                    .path("/file/batchCopy");
                then.status(200).json_body(json!({
                    "success": true,
                    "message": "复制成功"
                }));
            })
        }

        pub fn mock_upload_init(&self, file_id: &str) -> httpmock::Mock {
            self.server.mock(|when, then| {
                when.method(POST)
                    .path("/file/uploadInit");
                then.status(200).json_body(json!({
                    "success": true,
                    "data": {
                        "fileId": file_id,
                        "fileName": "test.txt",
                        "partInfos": [
                            {
                                "partNumber": 1,
                                "uploadUrl": "http://upload.example.com/part1"
                            }
                        ]
                    }
                }));
            })
        }
    }
}
