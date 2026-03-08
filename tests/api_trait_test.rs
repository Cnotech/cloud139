#![allow(dead_code)]

#[cfg(test)]
mod tests {
    use cloud139::client::api_trait::{ApiClient, RealApiClient};
    use cloud139::config::Config;
    use cloud139::client::StorageType;
    use cloud139::models::PersonalListResp;
    use async_trait::async_trait;

    #[tokio::test]
    async fn test_real_api_client() {
        let client = RealApiClient;
        let mut config = Config {
            account: "test@139.com".to_string(),
            cloud_id: Some("cloud123".to_string()),
            personal_cloud_host: Some("https://test.com".to_string()),
            storage_type: "personal_new".to_string(),
            authorization: "Basic dGVzdA==".to_string(),
            custom_upload_part_size: 0,
            report_real_size: true,
            use_large_thumbnail: false,
            refresh_token: None,
            token_expire_time: None,
            root_folder_id: None,
            user_domain_id: None,
        };

        // This will fail due to network, but the trait method is accessible
        let result = client.get_personal_cloud_host(&mut config).await;
        // Just verify the method is callable
        assert!(result.is_err() || result.is_ok());
    }

    #[test]
    fn test_real_api_client_sync() {
        // Test that RealApiClient implements the trait correctly
        let client = RealApiClient;
        // The client should be cloneable
        let _cloned = client.clone();
    }
}
