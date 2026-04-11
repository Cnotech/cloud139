use crate::client::ClientError;
use crate::client::StorageType;
use crate::config::Config;
use crate::models::*;
use async_trait::async_trait;

#[async_trait]
pub trait ApiClient: Send + Sync {
    async fn get_personal_cloud_host(&self, config: &mut Config) -> Result<String, ClientError>;

    async fn personal_api_request<T: for<'de> serde::Deserialize<'de> + Send>(
        &self,
        config: &Config,
        url: &str,
        body: serde_json::Value,
        storage_type: StorageType,
    ) -> Result<T, ClientError>;

    async fn get_file_id_by_path(&self, config: &Config, path: &str)
    -> Result<String, ClientError>;

    async fn check_file_exists(
        &self,
        config: &Config,
        parent_file_id: &str,
        file_name: &str,
    ) -> Result<bool, ClientError>;

    async fn list_personal_files(
        &self,
        config: &Config,
        parent_file_id: &str,
    ) -> Result<Vec<PersonalFileItem>, ClientError>;

    async fn get_personal_download_link(
        &self,
        config: &Config,
        file_id: &str,
    ) -> Result<String, ClientError>;

    async fn get_family_download_link(
        &self,
        config: &Config,
        content_id: &str,
        path: &str,
    ) -> Result<String, ClientError>;

    async fn get_group_download_link(
        &self,
        config: &Config,
        content_id: &str,
        path: &str,
    ) -> Result<String, ClientError>;
}

#[derive(Clone)]
pub struct RealApiClient;

#[async_trait]
impl ApiClient for RealApiClient {
    async fn get_personal_cloud_host(&self, config: &mut Config) -> Result<String, ClientError> {
        crate::client::api::get_personal_cloud_host(config).await
    }

    async fn personal_api_request<T: for<'de> serde::Deserialize<'de> + Send>(
        &self,
        config: &Config,
        url: &str,
        body: serde_json::Value,
        storage_type: StorageType,
    ) -> Result<T, ClientError> {
        crate::client::api::personal_api_request(config, url, body, storage_type).await
    }

    async fn get_file_id_by_path(
        &self,
        config: &Config,
        path: &str,
    ) -> Result<String, ClientError> {
        crate::client::api::get_file_id_by_path(config, path).await
    }

    async fn check_file_exists(
        &self,
        config: &Config,
        parent_file_id: &str,
        file_name: &str,
    ) -> Result<bool, ClientError> {
        crate::client::api::check_file_exists(config, parent_file_id, file_name).await
    }

    async fn list_personal_files(
        &self,
        config: &Config,
        parent_file_id: &str,
    ) -> Result<Vec<PersonalFileItem>, ClientError> {
        crate::client::api::list_personal_files(config, parent_file_id).await
    }

    async fn get_personal_download_link(
        &self,
        config: &Config,
        file_id: &str,
    ) -> Result<String, ClientError> {
        crate::client::api::get_personal_download_link(config, file_id).await
    }

    async fn get_family_download_link(
        &self,
        config: &Config,
        content_id: &str,
        path: &str,
    ) -> Result<String, ClientError> {
        crate::client::api::get_family_download_link(config, content_id, path).await
    }

    async fn get_group_download_link(
        &self,
        config: &Config,
        content_id: &str,
        path: &str,
    ) -> Result<String, ClientError> {
        crate::client::api::get_group_download_link(config, content_id, path).await
    }
}
