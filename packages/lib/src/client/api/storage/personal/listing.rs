use super::client::{ListingRetry, PersonalClient};
use crate::client::{ClientError, http::HttpClientWrapper};
use crate::config::Config;
use crate::debug;
use crate::models::{PersonalFileItem, PersonalListData, PersonalListResp};
use crate::protocol;
use crate::utils::api_response::require_success;
use futures_util::{Stream, TryStreamExt};
use std::{collections::HashSet, pin::pin};

#[derive(Clone, Copy)]
pub(crate) struct ListPageOptions {
    pub size: i32,
    pub thumbnails: Option<&'static [&'static str]>,
}

impl Default for ListPageOptions {
    fn default() -> Self {
        Self {
            size: 100,
            thumbnails: Some(protocol::STANDARD_THUMBNAILS),
        }
    }
}

impl PersonalClient<'_> {
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
        let mut seen = HashSet::from([protocol::ROOT_FILE_ID.to_owned()]);
        for part in path.split('/').filter(|name| !name.is_empty()) {
            let parent = match &current {
                Some(item) if item.is_folder() => item
                    .file_id
                    .as_deref()
                    .ok_or(ClientError::InvalidFilePath)?,
                Some(_) => return Err(invalid("父路径不是目录")),
                None => protocol::ROOT_FILE_ID,
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

    pub(crate) async fn find_child(
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

    pub(crate) async fn list_files(
        &self,
        parent_id: &str,
    ) -> Result<Vec<PersonalFileItem>, ClientError> {
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
            "orderBy": protocol::LIST_ORDER_FIELD,
            "orderDirection": protocol::LIST_ORDER_DIRECTION,
            "pageInfo": { "pageCursor": cursor, "pageSize": options.size },
            "parentFileId": parent_id,
        });
        if let Some(styles) = options.thumbnails {
            body["imageThumbnailStyleList"] = serde_json::json!(styles);
        }
        let request = || self.request::<PersonalListResp>(protocol::LIST_FILES, &body);
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

impl HttpClientWrapper {
    pub async fn check_file_exists(
        &self,
        config: &Config,
        parent_file_id: &str,
        file_name: &str,
    ) -> Result<bool, ClientError> {
        debug!(
            "检查云端文件是否存在 parent={} name={}",
            parent_file_id, file_name
        );
        let personal = PersonalClient::connect(config, self, ListingRetry::None).await?;
        let exists = personal
            .find_child(parent_file_id, file_name)
            .await?
            .is_some();
        debug!("已检查云端文件是否存在 result={}", exists);
        Ok(exists)
    }

    pub async fn list_personal_files(
        &self,
        config: &Config,
        parent_file_id: &str,
    ) -> Result<Vec<crate::models::PersonalFileItem>, ClientError> {
        PersonalClient::connect(config, self, ListingRetry::None)
            .await?
            .list_files(parent_file_id)
            .await
    }
}

pub async fn get_file_id_by_path(config: &Config, path: &str) -> Result<String, ClientError> {
    config.require_personal()?;
    let normalized = crate::utils::remote_path::normalize(path)?;
    let path = normalized.as_str();
    if path == "/" {
        debug!("云端根路径返回空文件 ID");
        return Ok(String::new());
    }

    let client = HttpClientWrapper::new();
    let listing = PersonalClient::connect(config, &client, ListingRetry::None).await?;
    listing
        .resolve_path(path)
        .await?
        .and_then(|item| item.file_id)
        .ok_or(ClientError::InvalidFilePath)
}
