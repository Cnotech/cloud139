use super::client::{ListingRetry, PersonalClient};
use crate::client::api::response::require_success;
use crate::client::http::HttpClientWrapper;
use crate::config::{Config, personal::PersonalConfig};
use crate::debug;
use crate::error::{ClientError, invalid};
use crate::models::list::{PersonalFileItem, PersonalListData, PersonalListResp};
use crate::protocol;
use futures_util::{Stream, TryStreamExt};
use std::{
    collections::{HashMap, HashSet},
    pin::pin,
};

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

struct DirectoryCursor {
    next: Option<String>,
    seen: HashSet<String>,
}

impl Default for DirectoryCursor {
    fn default() -> Self {
        Self {
            next: Some(String::new()),
            seen: HashSet::new(),
        }
    }
}

impl DirectoryCursor {
    async fn next_page(
        &mut self,
        client: &PersonalClient<'_>,
        parent_id: &str,
        options: ListPageOptions,
    ) -> Result<Option<Vec<PersonalFileItem>>, ClientError> {
        let Some(cursor) = self.next.take() else {
            return Ok(None);
        };
        if !self.seen.insert(cursor.clone()) {
            return Err(ClientError::Api("分页游标重复，已停止列目录".into()));
        }
        let page = client.page(parent_id, &cursor, options).await?;
        self.next = page.next_page_cursor.filter(|next| !next.is_empty());
        Ok(Some(page.items))
    }
}

#[derive(Default)]
struct ListedChildren {
    cursor: DirectoryCursor,
    by_name: HashMap<String, PersonalFileItem>,
}

/// 可跨多次调用复用的目录分页缓存。
#[derive(Default)]
pub(crate) struct PathLookupCache {
    directories: HashMap<String, ListedChildren>,
}

impl PathLookupCache {
    pub(crate) fn lookup<'a, 'config>(
        &'a mut self,
        client: &'a PersonalClient<'config>,
    ) -> PathLookup<'a, 'config> {
        PathLookup {
            client,
            directories: &mut self.directories,
        }
    }
}

/// 在一批只读预检中复用已经读取的目录分页。
pub(crate) struct PathLookup<'a, 'config> {
    client: &'a PersonalClient<'config>,
    directories: &'a mut HashMap<String, ListedChildren>,
}

impl<'a, 'config> PathLookup<'a, 'config> {
    pub(crate) async fn resolve_path(
        &mut self,
        path: &str,
    ) -> Result<Option<PersonalFileItem>, ClientError> {
        resolve_path_with(path, async |parent, name, _| {
            self.find_child(parent, name).await
        })
        .await
    }

    pub(crate) async fn find_child(
        &mut self,
        parent: &str,
        name: &str,
    ) -> Result<Option<PersonalFileItem>, ClientError> {
        let directory = self.directories.entry(parent.to_owned()).or_default();
        loop {
            if let Some(item) = directory.by_name.get(name) {
                return Ok(Some(item.clone()));
            }
            let Some(items) = directory
                .cursor
                .next_page(self.client, parent, ListPageOptions::default())
                .await?
            else {
                return Ok(None);
            };
            for item in items {
                if let Some(name) = &item.name {
                    directory.by_name.entry(name.clone()).or_insert(item);
                }
            }
        }
    }
}

async fn resolve_path_with(
    path: &str,
    mut find_child: impl AsyncFnMut(&str, &str, bool) -> Result<Option<PersonalFileItem>, ClientError>,
) -> Result<Option<PersonalFileItem>, ClientError> {
    if path.split('/').count() > 258 {
        return Err(invalid("目录层级过深"));
    }
    let mut current: Option<PersonalFileItem> = None;
    let mut seen = HashSet::from([protocol::ROOT_FILE_ID.to_owned()]);
    let mut parts = path.split('/').filter(|name| !name.is_empty()).peekable();
    while let Some(part) = parts.next() {
        let parent = match &current {
            Some(item) if item.is_folder() => item
                .file_id
                .as_deref()
                .ok_or(ClientError::InvalidFilePath)?,
            Some(_) => return Err(invalid("父路径不是目录")),
            None => protocol::ROOT_FILE_ID,
        };
        let item = find_child(parent, part, parts.peek().is_none())
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

impl PersonalClient<'_> {
    /// 解析规范化后的路径。根目录没有对应的列表条目。
    pub(crate) async fn resolve_path(
        &self,
        path: &str,
    ) -> Result<Option<PersonalFileItem>, ClientError> {
        resolve_path_with(path, async |parent, name, _| {
            self.find_child(parent, name).await
        })
        .await
    }

    /// 解析条目，并复用最后一级列表返回同级条目。
    pub(crate) async fn resolve_path_with_siblings(
        &self,
        path: &str,
    ) -> Result<(Option<PersonalFileItem>, Vec<PersonalFileItem>), ClientError> {
        let mut siblings = Vec::new();
        let item = resolve_path_with(path, async |parent, name, last| {
            if !last {
                return self.find_child(parent, name).await;
            }
            siblings = self.list_files(parent).await?;
            Ok(siblings
                .iter()
                .find(|item| item.name.as_deref() == Some(name))
                .cloned())
        })
        .await?;
        Ok((item, siblings))
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
        futures_util::stream::try_unfold(DirectoryCursor::default(), move |mut cursor| async move {
            let items = cursor.next_page(self, parent_id, options).await?;
            Ok(items.map(|items| (items, cursor)))
        })
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
        // 分页器推进游标前，先重试当前请求。
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
        let config = PersonalConfig::new(config)?;
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
    ) -> Result<Vec<crate::models::list::PersonalFileItem>, ClientError> {
        PersonalClient::connect(PersonalConfig::new(config)?, self, ListingRetry::None)
            .await?
            .list_files(parent_file_id)
            .await
    }
}
