use super::{DownloadLink, HttpClientWrapper, get_personal_cloud_host_with_client, personal_post};
use crate::client::ClientError;
use crate::config::Config;
use crate::domain::StorageType;
use crate::models::{PersonalFileItem, PersonalListData, PersonalListResp};
use futures_util::{Stream, TryStreamExt};
use std::{borrow::Cow, collections::HashSet, pin::pin, time::Duration};

pub(super) const CREATE_FILE: &str = "/file/create";

#[derive(Clone, Copy)]
pub(crate) enum ListingRetry {
    None,
    Transient {
        retries: u32,
        backoff_secs: u64,
        timeout: Option<Duration>,
    },
}

impl ListingRetry {
    pub(crate) const DIRECTORY: Self = Self::Transient {
        retries: 2,
        backoff_secs: 1,
        timeout: Some(Duration::from_secs(20)),
    };
}

#[derive(Clone, Copy)]
pub(crate) struct ListPageOptions {
    pub size: i32,
    pub thumbnails: Option<&'static [&'static str]>,
}

impl Default for ListPageOptions {
    fn default() -> Self {
        Self {
            size: 100,
            thumbnails: Some(&["Small", "Large"]),
        }
    }
}

pub(crate) struct PersonalClient<'a> {
    pub(super) config: &'a Config,
    host: Cow<'a, str>,
    pub(super) http_client: &'a HttpClientWrapper,
    retry: ListingRetry,
}

impl<'a> PersonalClient<'a> {
    pub(crate) async fn connect(
        config: &'a Config,
        client: &'a HttpClientWrapper,
        retry: ListingRetry,
    ) -> Result<Self, ClientError> {
        let host = get_personal_cloud_host_with_client(&mut config.clone(), client).await?;
        Ok(Self::with_host(config, host, client, retry))
    }

    pub(crate) fn with_host(
        config: &'a Config,
        host: impl Into<Cow<'a, str>>,
        http_client: &'a HttpClientWrapper,
        retry: ListingRetry,
    ) -> Self {
        Self {
            config,
            host: host.into(),
            http_client,
            retry,
        }
    }

    pub(crate) fn with_listing_retry(&self, retry: ListingRetry) -> PersonalClient<'_> {
        PersonalClient::with_host(self.config, self.host.as_ref(), self.http_client, retry)
    }

    pub(crate) fn host(&self) -> &str {
        &self.host
    }

    pub(super) async fn request<T: for<'de> serde::Deserialize<'de>>(
        &self,
        endpoint: &str,
        body: &serde_json::Value,
    ) -> Result<T, ClientError> {
        personal_post(
            self.config,
            &format!("{}{endpoint}", self.host),
            body,
            StorageType::PersonalNew,
            &self.http_client.client,
        )
        .await
    }

    pub(crate) async fn download_link(
        &self,
        file_id: &str,
    ) -> Result<DownloadLink<'_>, ClientError> {
        #[derive(serde::Deserialize)]
        struct Response {
            #[serde(flatten)]
            base: crate::models::BaseResp,
            data: Option<crate::models::DownloadUrlData>,
        }

        let response: Response = self
            .request(
                "/file/getDownloadUrl",
                &serde_json::json!({ "fileId": file_id }),
            )
            .await?;
        if !response.base.success {
            return Err(ClientError::Api(format!(
                "获取下载链接失败: {}",
                response.base.message.as_deref().unwrap_or("未知错误")
            )));
        }
        let data = response
            .data
            .ok_or_else(|| ClientError::Api("获取下载链接失败: 无数据".into()))?;
        let url = data
            .cdn_url
            .filter(|url| !url.is_empty())
            .or_else(|| data.url.filter(|url| !url.is_empty()))
            .ok_or_else(|| ClientError::Api("获取下载链接失败: URL为空".into()))?;
        Ok(DownloadLink {
            client: self.http_client,
            url,
            name: data.file_name,
        })
    }

    pub(crate) async fn file_detail(
        &self,
        file_id: &str,
    ) -> Result<crate::models::QueryFileData, ClientError> {
        let response: crate::models::QueryFileResp = self
            .request("/file/query", &serde_json::json!({ "fileId": file_id }))
            .await?;
        require_success(response.base, "获取文件详情失败")?;
        Ok(response.data)
    }

    pub(crate) async fn create_directory(
        &self,
        parent_id: &str,
        name: &str,
    ) -> Result<(), ClientError> {
        let response: crate::models::PersonalUploadResp = self
            .request(
                CREATE_FILE,
                &serde_json::json!({
                    "parentFileId": parent_id, "name": name, "description": "", "type": "folder"
                }),
            )
            .await?;
        require_success(response.base, "创建目录失败")
    }

    pub(crate) async fn rename(&self, file_id: &str, name: &str) -> Result<(), ClientError> {
        let response: crate::models::PersonalUploadResp = self
            .request(
                "/file/update",
                &serde_json::json!({ "fileId": file_id, "name": name, "description": "" }),
            )
            .await?;
        require_success(response.base, "重命名失败")
    }

    pub(crate) async fn trash(&self, file_id: &str) -> Result<(), ClientError> {
        let response: serde_json::Value = self
            .request(
                "/recyclebin/batchTrash",
                &serde_json::json!({ "fileIds": [file_id] }),
            )
            .await?;
        if !mutation_succeeded(&response) {
            return Err(ClientError::Api(
                response["message"]
                    .as_str()
                    .filter(|message| !message.is_empty())
                    .unwrap_or("删除失败：服务端未确认全部条目已移到回收站")
                    .to_owned(),
            ));
        }
        Ok(())
    }

    pub(crate) async fn move_files(
        &self,
        file_ids: &[String],
        target_id: &str,
    ) -> Result<(), ClientError> {
        self.transfer_files("/file/batchMove", file_ids, target_id)
            .await
    }

    pub(crate) async fn copy_files(
        &self,
        file_ids: &[String],
        target_id: &str,
    ) -> Result<(), ClientError> {
        self.transfer_files("/file/batchCopy", file_ids, target_id)
            .await
    }

    async fn transfer_files(
        &self,
        endpoint: &str,
        file_ids: &[String],
        target_id: &str,
    ) -> Result<(), ClientError> {
        let response: serde_json::Value = self
            .request(
                endpoint,
                &serde_json::json!({ "fileIds": file_ids, "toParentFileId": target_id }),
            )
            .await?;
        if !mutation_succeeded(&response) {
            return Err(ClientError::Api(
                "批量操作失败（可能部分完成），请检查云端状态".into(),
            ));
        }
        Ok(())
    }

    /// Resolve a normalized path. The root has no list item.
    pub(crate) async fn resolve_path(
        &self,
        path: &str,
    ) -> Result<Option<PersonalFileItem>, ClientError> {
        use crate::utils::remote_path::invalid;

        if path.split('/').count() > 258 {
            return Err(invalid("目录层级过深"));
        }
        let mut current: Option<PersonalFileItem> = None;
        let mut seen = HashSet::from(["/".to_owned()]);
        for part in path.split('/').filter(|name| !name.is_empty()) {
            let parent = match &current {
                Some(item) if item.is_folder() => item
                    .file_id
                    .as_deref()
                    .ok_or(ClientError::InvalidFilePath)?,
                Some(_) => return Err(invalid("父路径不是目录")),
                None => "/",
            };
            let item = self
                .find_child(parent, part)
                .await?
                .ok_or(ClientError::FileNotFound)?;
            let id = item
                .file_id
                .as_ref()
                .filter(|id| !id.is_empty())
                .ok_or(ClientError::InvalidFilePath)?;
            if !seen.insert(id.clone()) {
                return Err(invalid("路径解析返回根目录别名或目录循环"));
            }
            current = Some(item);
        }
        Ok(current)
    }

    pub async fn find_child(
        &self,
        parent_id: &str,
        name: &str,
    ) -> Result<Option<PersonalFileItem>, ClientError> {
        let mut pages = pin!(self.pages(parent_id, ListPageOptions::default()));
        while let Some(items) = pages.try_next().await? {
            if let Some(item) = items
                .into_iter()
                .find(|item| item.name.as_deref() == Some(name))
            {
                return Ok(Some(item));
            }
        }
        Ok(None)
    }

    pub async fn list_files(&self, parent_id: &str) -> Result<Vec<PersonalFileItem>, ClientError> {
        self.pages(parent_id, ListPageOptions::default())
            .try_fold(Vec::new(), |mut all, items| async {
                all.extend(items);
                Ok(all)
            })
            .await
    }

    pub(crate) fn pages<'b>(
        &'b self,
        parent_id: &'b str,
        options: ListPageOptions,
    ) -> impl Stream<Item = Result<Vec<PersonalFileItem>, ClientError>> + 'b {
        futures_util::stream::try_unfold(
            (Some(String::new()), HashSet::new()),
            move |(cursor, mut seen)| async move {
                let Some(cursor) = cursor else {
                    return Ok(None);
                };
                if !seen.insert(cursor.clone()) {
                    return Err(ClientError::Api("分页游标重复，已停止列目录".into()));
                }
                let page = self.page(parent_id, &cursor, options).await?;
                let next = page.next_page_cursor.filter(|next| !next.is_empty());
                Ok(Some((page.items, (next, seen))))
            },
        )
    }

    async fn page(
        &self,
        parent_id: &str,
        cursor: &str,
        options: ListPageOptions,
    ) -> Result<PersonalListData, ClientError> {
        let mut body = serde_json::json!({
            "orderBy": "updated_at",
            "orderDirection": "DESC",
            "pageInfo": { "pageCursor": cursor, "pageSize": options.size },
            "parentFileId": parent_id,
        });
        if let Some(styles) = options.thumbnails {
            body["imageThumbnailStyleList"] = serde_json::json!(styles);
        }
        let request = || self.request::<PersonalListResp>("/file/list", &body);
        // Retry this request before the pager advances the cursor.
        let response = match self.retry {
            ListingRetry::None => request().await?,
            ListingRetry::Transient {
                retries,
                backoff_secs,
                timeout,
            } => crate::utils::retry::with_retries(retries, backoff_secs, || async {
                if let Some(timeout) = timeout {
                    tokio::time::timeout(timeout, request())
                        .await
                        .map_err(|error| {
                            ClientError::Io(std::io::Error::new(
                                std::io::ErrorKind::TimedOut,
                                error,
                            ))
                        })?
                        .map_err(Into::into)
                } else {
                    request().await.map_err(Into::into)
                }
            })
            .await
            .map_err(|error| {
                error
                    .downcast::<ClientError>()
                    .unwrap_or_else(|error| ClientError::Other(format!("{error:#}")))
            })?,
        };
        require_success(response.base, "获取目录列表失败")?;
        response
            .data
            .ok_or_else(|| ClientError::Api("获取目录列表失败: 无数据".into()))
    }
}

fn require_success(
    response: crate::models::BaseResp,
    default_error: &str,
) -> Result<(), ClientError> {
    if !response.success {
        return Err(ClientError::Api(
            response.message.unwrap_or_else(|| default_error.to_owned()),
        ));
    }
    Ok(())
}

/// Some batch responses carry per-item success flags under data.
pub fn mutation_succeeded(response: &serde_json::Value) -> bool {
    fn no_failure(value: &serde_json::Value) -> bool {
        match value {
            serde_json::Value::Object(map) => {
                map.get("success").and_then(serde_json::Value::as_bool) != Some(false)
                    && map.values().all(no_failure)
            }
            serde_json::Value::Array(items) => items.iter().all(no_failure),
            _ => true,
        }
    }
    response.get("success").and_then(serde_json::Value::as_bool) == Some(true)
        && no_failure(response)
}
