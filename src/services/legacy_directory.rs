//! Family/group listing uses catalog IDs and server paths, never CLI paths as IDs.
//! Protocol reference: AlistGo/alist drivers/139 (see docs/directory-operations.md).
use super::directory_service::{invalid, validate_name};
use crate::{
    client::{
        Client, ClientError,
        endpoints::{family, group},
    },
    config::Config,
    domain::StorageType,
};
use serde_json::{Value, json};
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct LegacyEntry {
    pub id: String,
    pub name: String,
    pub folder: bool,
    pub wire_path: String,
    pub size: Option<i64>,
    pub modified: String,
}

pub fn check_response(value: &Value) -> Result<(), ClientError> {
    if value.get("success").and_then(Value::as_bool) == Some(false) {
        return Err(invalid("服务端报告操作失败"));
    }
    let result = value
        .get("result")
        .or_else(|| value.pointer("/data/result"));
    if result
        .and_then(|r| r.get("resultCode"))
        .and_then(Value::as_str)
        != Some("0")
    {
        return Err(ClientError::Api(
            "家庭/群组操作失败或响应缺少成功状态；请检查云端状态".into(),
        ));
    }
    Ok(())
}

pub fn root(config: &Config) -> Result<LegacyEntry, ClientError> {
    let family = config.storage_type() == StorageType::Family;
    let wire = config.root_folder_id.clone().filter(|p| !p.is_empty());
    if !family && wire.is_none() {
        return Err(invalid(
            "群组盘缺少已解析的 root_folder_id；拒绝猜测云端根路径",
        ));
    }
    let wire = wire.unwrap_or_default();
    // Family's empty catalog ID requests its root; group root uses the configured cloud ID.
    let id = if family {
        wire.rsplit('/')
            .next()
            .unwrap_or_default()
            .trim_start_matches("root:")
            .to_owned()
    } else {
        config
            .cloud_id
            .clone()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| invalid("群组盘缺少 cloud_id"))?
    };
    Ok(LegacyEntry {
        id,
        name: String::new(),
        folder: true,
        wire_path: wire,
        size: None,
        modified: String::new(),
    })
}

pub fn list_body(config: &Config, directory: &LegacyEntry, page: i32, size: i32) -> Value {
    if config.storage_type() == StorageType::Family {
        json!({"catalogID":directory.id,"contentSortType":0,"sortDirection":1,"pageInfo":{"pageNum":page,"pageSize":size}})
    } else {
        json!({"groupID":config.cloud_id,"catalogID":directory.id,"contentSortType":0,"sortDirection":1,
            "startNumber":(page - 1) * size + 1,"endNumber":page * size,"path":directory.wire_path})
    }
}

fn string(value: &Value, key: &str) -> Result<String, ClientError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| invalid(format!("服务端缺少 {key}")))
}

pub fn parse_page(
    config: &Config,
    value: &Value,
) -> Result<(Vec<LegacyEntry>, i64, String), ClientError> {
    check_response(value)?;
    let family = config.storage_type() == StorageType::Family;
    let data = if family {
        &value["data"]
    } else {
        &value["data"]["getGroupContentResult"]
    };
    let parent = data
        .get(if family { "path" } else { "parentCatalogID" })
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("服务端缺少父路径"))?
        .to_owned();
    let total = data[if family { "totalCount" } else { "nodeCount" }]
        .as_i64()
        .ok_or_else(|| invalid("服务端缺少条目数量"))?;
    if total < 0 {
        return Err(invalid("服务端条目数量无效"));
    }
    let mut entries = Vec::new();
    for (key, folder) in [
        (
            if family {
                "cloudCatalogList"
            } else {
                "catalogList"
            },
            true,
        ),
        (
            if family {
                "cloudContentList"
            } else {
                "contentList"
            },
            false,
        ),
    ] {
        let items = data[key]
            .as_array()
            .ok_or_else(|| invalid("服务端缺少目录列表"))?;
        for item in items {
            let id = string(item, if folder { "catalogID" } else { "contentID" })?;
            let name = string(item, if folder { "catalogName" } else { "contentName" })?;
            validate_name(&name)?;
            if parent.is_empty() && (family || !folder) {
                return Err(invalid("服务端返回空父路径"));
            }
            let wire_path = if !family && folder {
                string(item, "path")?
            } else {
                parent.clone()
            };
            entries.push(LegacyEntry {
                id,
                name,
                folder,
                wire_path,
                size: item["contentSize"].as_i64(),
                modified: item[if family {
                    "lastUpdateTime"
                } else {
                    "updateTime"
                }]
                .as_str()
                .unwrap_or_default()
                .into(),
            });
        }
    }
    Ok((entries, total, parent))
}

/// Injectable transport keeps protocol regression tests entirely on loopback.
#[async_trait::async_trait]
pub trait LegacyTransport: Sync {
    async fn request(&self, url: &str, body: Value) -> Result<Value, ClientError>;
}
pub struct SignedTransport {
    config: Config,
}
#[async_trait::async_trait]
impl LegacyTransport for SignedTransport {
    async fn request(&self, url: &str, body: Value) -> Result<Value, ClientError> {
        Client::new(self.config.clone())
            .api_request_post(url, body)
            .await
    }
}
pub struct LegacyBackend<'a, T: LegacyTransport> {
    pub config: &'a Config,
    pub transport: &'a T,
}

impl<T: LegacyTransport> LegacyBackend<'_, T> {
    pub async fn post(&self, url: &str, mut body: Value) -> Result<Value, ClientError> {
        let config = self.config;

        body["commonAccountInfo"] = json!({"account": config.account, "accountType": 1});
        if config.storage_type() == StorageType::Family {
            body["cloudID"] = json!(config.cloud_id);
            body["cloudType"] = json!(1);
        }
        let result: Value = self.transport.request(url, body).await?;
        check_response(&result)?;
        Ok(result)
    }
    pub async fn list_all(&self, dir: &LegacyEntry) -> Result<Vec<LegacyEntry>, ClientError> {
        let config = self.config;

        if !dir.folder {
            return Err(invalid("目标路径不是目录"));
        }
        let url = if config.storage_type() == StorageType::Family {
            family::orchestration::QUERY_CONTENT_LIST
        } else {
            group::orchestration::QUERY_GROUP_CONTENT_LIST
        };
        let mut entries = Vec::new();
        let mut ids = HashSet::new();
        let mut names = HashSet::new();
        for page in 1..=10000 {
            let value = self.post(url, list_body(config, dir, page, 100)).await?;
            let (items, total, _) = parse_page(config, &value)?;
            // Family totalCount can be the count on this page. Continue to an empty page.
            if items.is_empty() {
                if config.storage_type() == StorageType::Group && (entries.len() as i64) < total {
                    return Err(invalid("分页提前结束，未取得全部条目"));
                }
                return Ok(entries);
            }
            for item in items {
                if !ids.insert(item.id.clone()) || !names.insert(item.name.clone()) {
                    return Err(invalid("分页重复或目录名称冲突，已停止列目录"));
                }
                entries.push(item);
            }
            if config.storage_type() == StorageType::Group && entries.len() as i64 >= total {
                return Ok(entries);
            }
        }
        Err(invalid("分页超过安全上限"))
    }
    pub async fn resolve(&self, path: &str) -> Result<LegacyEntry, ClientError> {
        let normalized = super::directory_service::normalize(path)?;
        let path = normalized.as_str();
        if path.split('/').count() > 258 {
            return Err(invalid("目录层级过深"));
        }
        let config = self.config;

        let mut current = root(config)?;
        let mut seen_ids = HashSet::from([current.id.clone()]);
        for name in path.split('/').filter(|s| !s.is_empty()) {
            current = self
                .list_all(&current)
                .await?
                .into_iter()
                .find(|i| i.name == name)
                .ok_or(ClientError::FileNotFound)?;
            if !seen_ids.insert(current.id.clone()) {
                return Err(invalid("路径解析返回根目录别名或目录循环"));
            }
        }
        Ok(current)
    }
    pub async fn full_path(&self, entry: &LegacyEntry) -> Result<String, ClientError> {
        let config = self.config;

        if config.storage_type() == StorageType::Group {
            if entry.wire_path.is_empty() {
                return Err(invalid("服务端缺少目录路径"));
            }
            return Ok(entry.wire_path.clone());
        }
        if entry.name.is_empty() {
            let value = self
                .post(
                    family::orchestration::QUERY_CONTENT_LIST,
                    list_body(config, entry, 1, 100),
                )
                .await?;
            return string(&value["data"], "path");
        }
        Ok(format!(
            "{}/{}",
            entry.wire_path.trim_end_matches('/'),
            entry.id
        ))
    }
    pub async fn ensure_dir(&self, path: &str, parents: bool) -> Result<(), ClientError> {
        let normalized = super::directory_service::normalize(path)?;
        let path = normalized.as_str();
        if path == "/" {
            return if parents {
                Ok(())
            } else {
                Err(ClientError::CannotOperateOnRoot)
            };
        }
        let config = self.config;

        let mut current = root(config)?;
        let parts: Vec<_> = path.split('/').filter(|p| !p.is_empty()).collect();
        for (index, name) in parts.iter().enumerate() {
            let children = self.list_all(&current).await?;
            if let Some(entry) = children.into_iter().find(|i| i.name == *name) {
                if !entry.folder {
                    return Err(invalid(format!("已有同名文件: {name}")));
                }
                if index + 1 == parts.len() && !parents {
                    return Err(invalid("目录已存在"));
                }
                current = entry;
                continue;
            }
            if !parents && index + 1 != parts.len() {
                return Err(ClientError::FileNotFound);
            }
            let wire = self.full_path(&current).await?;
            let family = config.storage_type() == StorageType::Family;
            let (url, body) = if family {
                (
                    family::orchestration::CREATE_CLOUD_DOC,
                    json!({"docLibName":name,"path":wire}),
                )
            } else {
                (
                    group::orchestration::CREATE_GROUP_CATALOG,
                    json!({"catalogName":name,"parentFileId":current.id,"groupID":config.cloud_id,"path":wire}),
                )
            };
            self.post(url, body).await?;
            current = self
                .list_all(&current)
                .await?
                .into_iter()
                .find(|i| i.name == *name && i.folder)
                .ok_or_else(|| invalid("创建后无法确认目录名称；可能已部分完成"))?;
        }
        Ok(())
    }
    pub async fn rename(&self, path: &str, name: &str) -> Result<(), ClientError> {
        let normalized = super::directory_service::non_root(path)?;
        let path = normalized.as_str();
        validate_name(name)?;
        let config = self.config;

        let source = self.resolve(path).await?;
        if config.storage_type() == StorageType::Family && source.folder {
            return Err(ClientError::UnsupportedFamilyRenameFolder);
        }
        if source.name == name {
            return Ok(());
        }
        let parent = path.rsplit_once('/').unwrap().0;
        let parent = self.resolve(parent).await?;
        if self.list_all(&parent).await?.iter().any(|i| i.name == name) {
            return Err(invalid("已有同名条目，拒绝重命名"));
        }
        let (url, body) = match (config.storage_type(), source.folder) {
            (StorageType::Family, false) => (
                family::orchestration::MODIFY_CONTENT_INFO,
                json!({"contentID":source.id,"contentName":name,"path":source.wire_path}),
            ),
            (StorageType::Group, true) => (
                group::orchestration::MODIFY_GROUP_CATALOG,
                json!({"groupID":config.cloud_id,"modifyCatalogID":source.id,"modifyCatalogName":name,"path":source.wire_path}),
            ),
            (StorageType::Group, false) => (
                group::orchestration::MODIFY_GROUP_CONTENT,
                json!({"groupID":config.cloud_id,"contentID":source.id,"contentName":name,"path":source.wire_path}),
            ),
            _ => return Err(invalid("不支持该重命名操作")),
        };
        self.post(url, body).await?;
        Ok(())
    }
    pub async fn trash(&self, path: &str, recursive: bool) -> Result<(), ClientError> {
        let normalized = super::directory_service::non_root(path)?;
        let path = normalized.as_str();
        let config = self.config;

        let entry = self.resolve(path).await?;
        if entry.folder && !recursive {
            return Err(invalid("删除目录需要显式指定 -r / --recursive"));
        }
        let family = config.storage_type() == StorageType::Family;
        let object = if family {
            entry.id.clone()
        } else if entry.folder {
            entry.wire_path.clone()
        } else {
            format!("{}/{}", entry.wire_path.trim_end_matches('/'), entry.id)
        };
        let mut body = json!({"taskType":2,"catalogList":if entry.folder { vec![object.clone()] } else { vec![] },"contentList":if entry.folder { vec![] } else { vec![object] }});
        let url = if family {
            body["sourceCloudID"] = json!(config.cloud_id);
            body["sourceCatalogType"] = json!(1002);
            body["path"] = json!(entry.wire_path);
            family::orchestration::CREATE_BATCH_OPR_TASK
        } else {
            body["srcGroupID"] = json!(config.cloud_id);
            group::orchestration::CREATE_BATCH_OPR_TASK
        };
        self.post(url, body).await?;
        crate::info!("删除任务已接受，请通过 ls 核对结果");
        Ok(())
    }
    pub async fn move_group(&self, sources: &[String], target: &str) -> Result<(), ClientError> {
        let config = self.config;

        if config.storage_type() != StorageType::Group {
            return Err(invalid("家庭盘移动协议未核实，已拒绝操作"));
        }
        if sources.len() != 1 {
            return Err(ClientError::UnsupportedGroupBatchMove);
        }
        let source_path = super::directory_service::non_root(&sources[0])?;
        let target_path = super::directory_service::normalize(target)?;
        let target = target_path.as_str();
        if target == source_path || target.starts_with(&format!("{source_path}/")) {
            return Err(invalid("不能移动到自身或子目录"));
        }
        let source = self.resolve(&source_path).await?;
        let dest = self.resolve(target).await?;
        if !dest.folder {
            return Err(invalid("目标路径不是目录"));
        }
        if source.id == dest.id {
            return Err(invalid("不能移动到自身"));
        }
        if self
            .list_all(&dest)
            .await?
            .iter()
            .any(|i| i.name == source.name)
        {
            return Err(invalid("目标已有同名条目"));
        }
        let wire = self.full_path(&dest).await?;
        self.post(group::orchestration::CREATE_BATCH_OPR_TASK,json!({"taskType":3,"srcType":2,"srcGroupID":config.cloud_id,"destType":2,"destGroupID":config.cloud_id,"destPath":wire,
        "contentList":if source.folder {vec![]} else {vec![source.id.clone()]},"catalogList":if source.folder {vec![source.id]} else {vec![]}})).await?;
        crate::info!("移动任务已接受，请通过 ls 核对结果");
        Ok(())
    }
}
pub async fn post(config: &Config, url: &str, body: Value) -> Result<Value, ClientError> {
    let transport = SignedTransport {
        config: config.clone(),
    };
    LegacyBackend {
        config,
        transport: &transport,
    }
    .post(url, body)
    .await
}
pub async fn list_all(config: &Config, dir: &LegacyEntry) -> Result<Vec<LegacyEntry>, ClientError> {
    let transport = SignedTransport {
        config: config.clone(),
    };
    LegacyBackend {
        config,
        transport: &transport,
    }
    .list_all(dir)
    .await
}
pub async fn resolve(config: &Config, path: &str) -> Result<LegacyEntry, ClientError> {
    let transport = SignedTransport {
        config: config.clone(),
    };
    LegacyBackend {
        config,
        transport: &transport,
    }
    .resolve(path)
    .await
}
pub async fn full_path(config: &Config, entry: &LegacyEntry) -> Result<String, ClientError> {
    let transport = SignedTransport {
        config: config.clone(),
    };
    LegacyBackend {
        config,
        transport: &transport,
    }
    .full_path(entry)
    .await
}
pub async fn ensure_dir(config: &Config, path: &str, parents: bool) -> Result<(), ClientError> {
    let transport = SignedTransport {
        config: config.clone(),
    };
    LegacyBackend {
        config,
        transport: &transport,
    }
    .ensure_dir(path, parents)
    .await
}
pub async fn rename(config: &Config, path: &str, name: &str) -> Result<(), ClientError> {
    let transport = SignedTransport {
        config: config.clone(),
    };
    LegacyBackend {
        config,
        transport: &transport,
    }
    .rename(path, name)
    .await
}
pub async fn trash(config: &Config, path: &str, recursive: bool) -> Result<(), ClientError> {
    let transport = SignedTransport {
        config: config.clone(),
    };
    LegacyBackend {
        config,
        transport: &transport,
    }
    .trash(path, recursive)
    .await
}
pub async fn move_group(
    config: &Config,
    sources: &[String],
    target: &str,
) -> Result<(), ClientError> {
    let transport = SignedTransport {
        config: config.clone(),
    };
    LegacyBackend {
        config,
        transport: &transport,
    }
    .move_group(sources, target)
    .await
}
