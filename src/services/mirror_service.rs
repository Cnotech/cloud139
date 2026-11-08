//! `mirror`: 把本地目录镜像到云端（补齐 → 校验 → 可选删除），进程内完成。
//!
//! 设计约束：
//! - **默认只补缺失**：远端多出来的文件不动；只有显式 `--delete` 才删，使远端 = 本地。
//! - **删除集合在任何上传之前算好并冻结**（来源是第一次远端扫描），上传阶段不重算；
//!   本轮上传的都来自本地，天然不在删除集合里，所以不需要删除前重列。
//! - **只有全部校验通过才删**：任何上传/校验失败 → 一个都不删、退出码 1。
//! - 唯一保护：本地目录不存在（参数校验）或为空（`--delete` 时）→ 退出码 2 且不删；
//!   `--no-verify --delete` 直接拒绝（没有「校验通过」这个前提）。
//! - 远端根目录由本命令逐级创建（cloud139 的 mkdir 没有 `-p`，同名加 `-f` 会改名），禁止 `-f`。
//! - 校验按 size + contentHash（统一小写比对）；缺哈希只计入 hashMissing，不判失败。
//! - 失败也要输出完整 JSON，由调用方写 latest.json。

use crate::client::ClientError;
use crate::client::api::{self, HttpClientWrapper};
use crate::config::Config;
use crate::domain::SyncEntryKind;
use crate::models::PersonalFileItem;
use crate::services::sync_service::{SyncScanOptions, parse_sync_endpoint, scan_local};
use crate::{debug, info, warn};
use anyhow::{Context, Result, anyhow};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct MirrorOptions {
    pub verify: bool,
    pub heal: u32,
    pub dry_run: bool,
    /// 删除「远端有、本地没有」的文件，使远端 = 本地。
    pub delete: bool,
    pub page_size: i32,
    pub retries: u32,
    pub retry_backoff_secs: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MirrorFailure {
    pub path: String,
    pub error: String,
    pub retries: u32,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MirrorReport {
    pub objects: usize,
    pub uploaded: usize,
    pub skipped: usize,
    /// `--delete` 时实际删除的文件数；`--dry-run` 时是「计划删除数」。
    pub deleted: usize,
    pub hash_missing: usize,
    pub seconds: f64,
    pub failures: Vec<MirrorFailure>,
}

impl MirrorReport {
    pub fn ok(&self) -> bool {
        self.failures.is_empty()
    }
}

/// 前置条件不满足：调用方按 exit 2 处理（参数/配置错误），不是对象传输失败。
#[derive(Debug)]
pub struct PreconditionError(pub String);

impl std::fmt::Display for PreconditionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for PreconditionError {}

#[derive(Debug, Clone)]
struct LocalObject {
    rel_path: String,
    abs_path: PathBuf,
    size: u64,
    /// 期望的内容 sha256：内容寻址对象用文件名，其它文件现算本地文件。
    expected_sha256: String,
}

/// 普通文件同名不同内容：拒绝覆盖（路径配错时靠这条拦住，不能写坏别人的仓）。
const ERR_CONTENT_DIFFERS: &str = "远端同名但内容不同，拒绝覆盖";
/// 普通文件拿不到远端哈希：无法证明相同，按失败处理，不能静默放过。
const ERR_HASH_UNAVAILABLE: &str = "远端未返回 contentHash，无法确认内容相同";

#[derive(Debug, Clone)]
struct RemoteObject {
    file_id: String,
    size: i64,
    content_hash: Option<String>,
    content_hash_algorithm: Option<String>,
    is_folder: bool,
}

pub async fn run(
    config: &Config,
    local_root: &str,
    remote_root: &str,
    options: &MirrorOptions,
) -> Result<MirrorReport> {
    let started = Instant::now();
    let mut report = MirrorReport::default();

    let remote_root = match parse_sync_endpoint(remote_root) {
        crate::domain::SyncEndpoint::Cloud(path) => path,
        crate::domain::SyncEndpoint::Local(_) => {
            return Err(anyhow!("远端必须是 cloud:/path 形式"));
        }
    };
    let local_root = PathBuf::from(local_root);
    let locals = scan_local_objects(&local_root)?;
    report.objects = locals.len();

    // 唯一的「本地为空」保护：只在 --delete 时才需要（不删的时候空目录无害）。
    if options.delete && locals.is_empty() {
        return Err(anyhow::Error::new(PreconditionError(format!(
            "本地目录为空，拒绝执行 --delete（不做任何删除）: {}",
            local_root.display()
        ))));
    }

    if options.dry_run {
        // --dry-run：只算差异，不发任何写请求（远端根也不建）。
        let remote = match scan_remote(config, &remote_root, options).await {
            Ok(remote) => remote,
            Err(err) if is_not_found(&err) => BTreeMap::new(),
            Err(err) => return Err(err),
        };
        // --dry-run 用与正式跑完全相同的冻结集合，所以这里报的 deleted 就是会删的那批。
        let delete_set = plan_delete_set(&remote, &locals, options.delete);
        report.deleted = delete_set.len();
        for local in &locals {
            let entry = match remote.get(&local.rel_path) {
                Some(entry) => entry,
                None => {
                    report.uploaded += 1;
                    continue;
                }
            };
            match classify_existing(local, entry, options.verify) {
                ExistingAction::Unchanged => report.skipped += 1,
                ExistingAction::UnchangedHashMissing => {
                    // 口径与正式跑一致：--dry-run 也要反映「有多少对象只能靠文件名保证」。
                    report.skipped += 1;
                    report.hash_missing += 1;
                }
                // --dry-run 只报告「正式跑会失败」，便于一次看全，不中断。
                ExistingAction::Fail(error) => report.failures.push(MirrorFailure {
                    path: local.rel_path.clone(),
                    error,
                    retries: 0,
                }),
                ExistingAction::Reupload => report.uploaded += 1,
            }
        }
        report.seconds = started.elapsed().as_secs_f64();
        return Ok(report);
    }

    let mut ensured = std::collections::HashSet::new();
    ensure_path(config, &remote_root, options, &mut ensured).await?;
    let remote = scan_remote(config, &remote_root, options).await?;
    // 删除集合在两次扫描都成功之后立刻算好并冻结；后面只读它，不重算、不再看远端。
    let delete_set = plan_delete_set(&remote, &locals, options.delete);

    for local in &locals {
        let remote_path = join_remote(&remote_root, &local.rel_path);
        let existing = remote.get(&local.rel_path);

        if let Some(entry) = existing {
            match classify_existing(local, entry, options.verify) {
                ExistingAction::Unchanged => {
                    report.skipped += 1;
                    continue;
                }
                ExistingAction::UnchangedHashMissing => {
                    report.skipped += 1;
                    report.hash_missing += 1;
                    continue;
                }
                // 「整轮失败」= 记一条失败后继续处理其它对象，最后统一返回 1。
                ExistingAction::Fail(error) => {
                    report.failures.push(MirrorFailure {
                        path: local.rel_path.clone(),
                        error,
                        retries: 0,
                    });
                    continue;
                }
                ExistingAction::Reupload => {
                    // 内容寻址对象同名必然同内容，走到这里说明远端那份坏了：
                    // 先永久删掉旧的，再重传，避免同名冲突。
                    warn!("远端对象与本地不一致，删除后重传: {}", remote_path);
                    if let Err(err) = crate::utils::retry::with_retries(
                        options.retries,
                        options.retry_backoff_secs,
                        || async {
                            crate::services::delete_service::delete(config, &remote_path, true)
                                .await
                        },
                    )
                    .await
                    {
                        report.failures.push(MirrorFailure {
                            path: local.rel_path.clone(),
                            error: format!("删除旧对象失败: {err:#}"),
                            retries: options.retries,
                        });
                        continue;
                    }
                }
            }
        }

        match upload_one(config, local, &remote_root, options, &mut ensured).await {
            Ok(_) => {
                report.uploaded += 1;
                if options.verify {
                    match verify_one(config, &remote_root, local, options).await {
                        Ok(ExistingAction::Unchanged) => {}
                        Ok(ExistingAction::UnchangedHashMissing) => report.hash_missing += 1,
                        // 普通文件拿不到哈希：刚传上去也证明不了内容相同，按失败处理。
                        Ok(ExistingAction::Fail(error)) => {
                            report.failures.push(MirrorFailure {
                                path: local.rel_path.clone(),
                                error,
                                retries: 0,
                            });
                        }
                        Ok(ExistingAction::Reupload) | Err(_) => {
                            // 自愈：删远端→重传→复核。此刻对象刚由本进程创建，
                            // 删重传不会动到别人的数据（同名普通文件的冲突已在上面拦掉）。
                            let mut healed = false;
                            for attempt in 0..options.heal {
                                debug!(
                                    "自愈第 {}/{} 次: {}",
                                    attempt + 1,
                                    options.heal,
                                    local.rel_path
                                );
                                if heal_one(config, &remote_root, local, options, &mut ensured)
                                    .await
                                    .is_ok()
                                {
                                    healed = true;
                                    break;
                                }
                            }
                            if !healed {
                                report.failures.push(MirrorFailure {
                                    path: local.rel_path.clone(),
                                    error: "校验失败且自愈未成功".to_string(),
                                    retries: options.heal,
                                });
                            }
                        }
                    }
                }
            }
            Err(err) => report.failures.push(MirrorFailure {
                path: local.rel_path.clone(),
                error: format!("{err:#}"),
                retries: options.retries,
            }),
        }
    }

    // 删除阶段：只有全部校验通过才动远端。
    if !delete_set.is_empty() {
        if report.failures.is_empty() {
            info!("本轮计划删除 {} 个远端文件", delete_set.len());
            let (deleted, failures) =
                delete_extra_objects(config, &remote_root, &delete_set, options).await;
            report.deleted = deleted;
            report.failures.extend(failures);
        } else {
            info!(
                "有 {} 个对象处理失败，本轮不执行删除（计划删除 {} 个）",
                report.failures.len(),
                delete_set.len()
            );
        }
    }

    report.seconds = started.elapsed().as_secs_f64();
    Ok(report)
}

/// 「远端有、本地没有」的文件（不含目录）。executor: 这是删除集合，调用方在两次扫描后立刻冻结。
fn plan_delete_set(
    remote: &BTreeMap<String, RemoteObject>,
    locals: &[LocalObject],
    delete: bool,
) -> Vec<String> {
    if !delete {
        return Vec::new();
    }
    let local_paths: std::collections::HashSet<&str> =
        locals.iter().map(|item| item.rel_path.as_str()).collect();
    remote
        .iter()
        .filter(|(path, item)| !item.is_folder && !local_paths.contains(path.as_str()))
        .map(|(path, _)| path.clone())
        .collect()
}

/// 逐个删除；失败的继续删其它并记录，返回（成功删除数, 失败列表）。
async fn delete_extra_objects(
    config: &Config,
    remote_root: &str,
    delete_set: &[String],
    options: &MirrorOptions,
) -> (usize, Vec<MirrorFailure>) {
    let mut deleted = 0;
    let mut failures = Vec::new();
    for rel_path in delete_set {
        let remote_path = join_remote(remote_root, rel_path);
        match crate::utils::retry::with_retries(
            options.retries,
            options.retry_backoff_secs,
            || async { crate::services::delete_service::delete(config, &remote_path, true).await },
        )
        .await
        {
            Ok(()) => {
                debug!("已删除远端多余文件: {}", remote_path);
                deleted += 1;
            }
            Err(err) => failures.push(MirrorFailure {
                path: rel_path.clone(),
                error: format!("删除失败: {err:#}"),
                retries: options.retries,
            }),
        }
    }
    (deleted, failures)
}

fn scan_local_objects(root: &Path) -> Result<Vec<LocalObject>> {
    if !root.is_dir() {
        return Err(anyhow!("本地仓根不是目录: {}", root.display()));
    }
    let entries = scan_local(
        root,
        SyncScanOptions {
            recursive: true,
            exclude: Vec::new(),
        },
    )
    .context("扫描本地仓失败")?;

    let mut objects = Vec::new();
    for entry in entries {
        if entry.kind != SyncEntryKind::File {
            continue;
        }
        let abs_path = entry
            .rel_path
            .split('/')
            .filter(|part| !part.is_empty())
            .fold(root.to_path_buf(), |path, part| path.join(part));
        let expected_sha256 = expected_sha256(&entry.rel_path, &abs_path)?;
        objects.push(LocalObject {
            rel_path: entry.rel_path,
            abs_path,
            size: entry.size,
            expected_sha256,
        });
    }
    Ok(objects)
}

/// 文件名是 64 位十六进制 → 内容寻址对象，名字本身就是内容 sha256。
///
/// 这条规则不写死任何仓的类型：内容寻址的对象（数据块、index、keys、snapshots）
/// 都天然满足，非内容寻址的（例如 restic 的 `config`）自然落到「普通文件」。
fn is_content_addressed(rel_path: &str) -> bool {
    let name = rel_path.rsplit('/').next().unwrap_or(rel_path);
    name.len() == 64 && name.chars().all(|c| c.is_ascii_hexdigit())
}

/// 期望的内容 sha256：内容寻址对象用文件名（统一小写），其它文件现算本地 sha256。
fn expected_sha256(rel_path: &str, abs_path: &Path) -> Result<String> {
    if is_content_addressed(rel_path) {
        let name = rel_path.rsplit('/').next().unwrap_or(rel_path);
        return Ok(name.to_lowercase());
    }
    let hash = crate::utils::crypto::calc_file_sha256(&abs_path.to_string_lossy())
        .map_err(|err| anyhow!("计算 {} 的 sha256 失败: {err}", rel_path))?;
    Ok(hash.to_lowercase())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HashCheck {
    Match,
    Mismatch,
    /// 远端没给哈希，或算法不是 SHA256——无法比对，按「缺哈希」处理，不判失败。
    Unavailable,
}

fn is_sha256_algorithm(algorithm: Option<&str>) -> bool {
    algorithm
        .map(|value| value.trim().eq_ignore_ascii_case("SHA256"))
        .unwrap_or(false)
}

/// 远端给了可比的 SHA256（有哈希且算法是 SHA256）。
fn has_comparable_hash(remote: &RemoteObject) -> bool {
    remote.content_hash.is_some() && is_sha256_algorithm(remote.content_hash_algorithm.as_deref())
}

fn check_hash(remote_hash: Option<&str>, algorithm: Option<&str>, expected: &str) -> HashCheck {
    match remote_hash {
        Some(actual) if is_sha256_algorithm(algorithm) => {
            if hash_matches(actual, expected) {
                HashCheck::Match
            } else {
                HashCheck::Mismatch
            }
        }
        _ => HashCheck::Unavailable,
    }
}

/// 远端独有 = 远端有、本地没有的**文件**（目录不算，目录只是容器的骨架）。
fn needs_upload(local: &LocalObject, remote: &RemoteObject, verify: bool) -> bool {
    if remote.is_folder {
        return true;
    }
    if remote.size != local.size as i64 {
        return true;
    }
    if !verify {
        return false;
    }
    matches!(
        check_hash(
            remote.content_hash.as_deref(),
            remote.content_hash_algorithm.as_deref(),
            &local.expected_sha256
        ),
        HashCheck::Mismatch
    )
}

/// 远端已有同名对象时的处置。dry-run、正式运行、上传后复核共用这一处判据。
#[derive(Debug, Clone, PartialEq, Eq)]
enum ExistingAction {
    /// 已确认一致，跳过（verify 开启时计入 verified）。
    Unchanged,
    /// 远端没给 SHA256，但对象是文件名寻址的，有第二层保证：跳过并计入 hashMissing。
    UnchangedHashMissing,
    /// 不能确认或已确认不同：记一条失败，绝不动远端。
    Fail(String),
    /// 内容寻址对象同名不同内容（远端那份坏了）：删掉重传。
    Reupload,
}

fn classify_existing(local: &LocalObject, remote: &RemoteObject, verify: bool) -> ExistingAction {
    let content_addressed = is_content_addressed(&local.rel_path);
    // 同名的是目录，或大小/哈希对不上，都是冲突。
    if remote.is_folder || needs_upload(local, remote, verify) {
        return if content_addressed {
            ExistingAction::Reupload
        } else {
            ExistingAction::Fail(ERR_CONTENT_DIFFERS.to_string())
        };
    }
    if verify && !has_comparable_hash(remote) {
        // 内容寻址对象还有「名字=哈希」这层保证；普通文件没有，缺哈希就不能当相同放过。
        return if content_addressed {
            ExistingAction::UnchangedHashMissing
        } else {
            ExistingAction::Fail(ERR_HASH_UNAVAILABLE.to_string())
        };
    }
    ExistingAction::Unchanged
}

fn hash_matches(remote_hash: &str, expected: &str) -> bool {
    remote_hash.trim().to_lowercase() == expected.trim().to_lowercase()
}

fn join_remote(root: &str, rel_path: &str) -> String {
    if root == "/" {
        format!("/{}", rel_path.trim_start_matches('/'))
    } else {
        format!(
            "{}/{}",
            root.trim_end_matches('/'),
            rel_path.trim_start_matches('/')
        )
    }
}

fn is_already_exists(err: &ClientError) -> bool {
    match err {
        ClientError::Api(message) | ClientError::Other(message) => {
            message.contains("已存在") || message.contains("--force")
        }
        _ => false,
    }
}

fn is_not_found(err: &anyhow::Error) -> bool {
    err.chain().any(|cause| {
        matches!(
            cause.downcast_ref::<ClientError>(),
            Some(ClientError::Api(message)) if message.contains("not found")
        )
    })
}

async fn ensure_path(
    config: &Config,
    path: &str,
    options: &MirrorOptions,
    ensured: &mut std::collections::HashSet<String>,
) -> Result<()> {
    if path == "/" {
        return Ok(());
    }
    let mut prefix = String::new();
    for segment in path.trim_matches('/').split('/') {
        if segment.is_empty() {
            continue;
        }
        let parent = if prefix.is_empty() {
            "/".to_string()
        } else {
            prefix.clone()
        };
        let next_prefix = if prefix.is_empty() {
            format!("/{segment}")
        } else {
            format!("{prefix}/{segment}")
        };
        if ensured.contains(&next_prefix) {
            prefix = next_prefix;
            continue;
        }
        let exists = crate::utils::retry::with_retries(
            options.retries,
            options.retry_backoff_secs,
            || async {
                // 列表接口要的是 parentFileId，不能直接传路径（只有 "/" 例外）。
                let parent_id = if parent == "/" {
                    "/".to_string()
                } else {
                    api::get_file_id_by_path(config, &parent).await?
                };
                let items = api::list_personal_files_with_page_size(
                    config,
                    &parent_id,
                    options.page_size,
                    &HttpClientWrapper::new(),
                )
                .await
                .map_err(anyhow::Error::from)?;
                let names: Vec<String> =
                    items.iter().filter_map(|item| item.name.clone()).collect();
                debug!("mirror: 列 {parent} 得到 {} 项: {:?}", names.len(), names);
                Ok(items
                    .iter()
                    .any(|item| item.name.as_deref() == Some(segment) && is_folder(item)))
            },
        )
        .await?;

        prefix = next_prefix;
        debug!("mirror: 远端目录 {prefix} 存在={exists}");
        if !exists {
            // 逐级建，禁止 -f：加 -f 会在同名时改名建目录。
            // 列表可能有延迟，mkdir 报「已存在」时视为成功（幂等），其它错误才重试。
            let created = crate::utils::retry::with_retries(
                options.retries,
                options.retry_backoff_secs,
                || async {
                    match crate::services::mkdir_service::mkdir(config, &prefix, false).await {
                        Ok(()) => Ok(()),
                        Err(err) if is_already_exists(&err) => Ok(()),
                        Err(err) => Err(anyhow::Error::from(err)),
                    }
                },
            )
            .await;
            created.with_context(|| format!("创建远端目录失败: {prefix}"))?;
        }
        ensured.insert(prefix.clone());
    }
    Ok(())
}

async fn scan_remote(
    config: &Config,
    remote_root: &str,
    options: &MirrorOptions,
) -> Result<BTreeMap<String, RemoteObject>> {
    let mut out = BTreeMap::new();
    let root_id = if remote_root == "/" {
        "/".to_string()
    } else {
        api::get_file_id_by_path(config, remote_root).await?
    };
    scan_remote_dir(config, remote_root, "", &root_id, options, &mut out).await?;
    Ok(out)
}

async fn scan_remote_dir(
    config: &Config,
    remote_root: &str,
    rel_parent: &str,
    parent_id: &str,
    options: &MirrorOptions,
    out: &mut BTreeMap<String, RemoteObject>,
) -> Result<()> {
    let items =
        crate::utils::retry::with_retries(options.retries, options.retry_backoff_secs, || async {
            api::list_personal_files_with_page_size(
                config,
                parent_id,
                options.page_size,
                &HttpClientWrapper::new(),
            )
            .await
            .map_err(anyhow::Error::from)
        })
        .await?;

    for item in &items {
        let name = match item.name.as_deref() {
            Some(name) if !name.is_empty() => name,
            _ => continue,
        };
        let rel_path = if rel_parent.is_empty() {
            name.to_string()
        } else {
            format!("{rel_parent}/{name}")
        };
        let entry = RemoteObject {
            file_id: item.file_id.clone().unwrap_or_default(),
            size: item.size.unwrap_or(0),
            content_hash: item.content_hash.clone(),
            content_hash_algorithm: item.content_hash_algorithm.clone(),
            is_folder: is_folder(item),
        };
        let is_dir = entry.is_folder;
        let child_id = entry.file_id.clone();
        out.insert(rel_path.clone(), entry);
        if is_dir && !child_id.is_empty() {
            Box::pin(scan_remote_dir(
                config,
                remote_root,
                &rel_path,
                &child_id,
                options,
                out,
            ))
            .await?;
        }
    }
    let _ = remote_root;
    Ok(())
}

fn is_folder(item: &PersonalFileItem) -> bool {
    crate::services::sync_service::should_treat_personal_item_as_folder(item)
}

async fn upload_one(
    config: &Config,
    local: &LocalObject,
    remote_root: &str,
    options: &MirrorOptions,
    ensured: &mut std::collections::HashSet<String>,
) -> Result<u64> {
    let parent = parent_remote_path(&join_remote(remote_root, &local.rel_path));
    ensure_path(config, &parent, options, ensured).await?;
    let file_name = local
        .rel_path
        .rsplit('/')
        .next()
        .ok_or_else(|| anyhow!("无法得到文件名: {}", local.rel_path))?
        .to_string();
    let size = local.size as i64;

    crate::utils::retry::with_retries(options.retries, options.retry_backoff_secs, || async {
        crate::services::upload_service::upload(
            config,
            &local.abs_path,
            &parent,
            &file_name,
            size,
            false,
            None,
        )
        .await
        .map_err(anyhow::Error::from)
    })
    .await?;
    Ok(local.size)
}

fn parent_remote_path(path: &str) -> String {
    match path.rfind('/') {
        Some(0) => "/".to_string(),
        Some(index) => path[..index].to_string(),
        None => "/".to_string(),
    }
}

/// 上传后复核：把远端这份按同一套判据再分类一次。
async fn verify_one(
    config: &Config,
    remote_root: &str,
    local: &LocalObject,
    options: &MirrorOptions,
) -> Result<ExistingAction> {
    let remote_path = join_remote(remote_root, &local.rel_path);
    let parent = parent_remote_path(&remote_path);
    let file_name = local
        .rel_path
        .rsplit('/')
        .next()
        .unwrap_or(&local.rel_path)
        .to_string();
    let parent_id = api::get_file_id_by_path(config, &parent).await?;
    let items =
        crate::utils::retry::with_retries(options.retries, options.retry_backoff_secs, || async {
            api::list_personal_files_with_page_size(
                config,
                &parent_id,
                options.page_size,
                &HttpClientWrapper::new(),
            )
            .await
            .map_err(anyhow::Error::from)
        })
        .await?;

    let item = items
        .iter()
        .find(|item| item.name.as_deref() == Some(file_name.as_str()))
        .ok_or_else(|| anyhow!("上传后远端找不到对象: {remote_path}"))?;

    let remote = RemoteObject {
        file_id: item.file_id.clone().unwrap_or_default(),
        size: item.size.unwrap_or(0),
        content_hash: item.content_hash.clone(),
        content_hash_algorithm: item.content_hash_algorithm.clone(),
        is_folder: is_folder(item),
    };
    // verify=true：刚上传成功，内容必须能证明与本地一致。
    Ok(classify_existing(local, &remote, true))
}

async fn heal_one(
    config: &Config,
    remote_root: &str,
    local: &LocalObject,
    options: &MirrorOptions,
    ensured: &mut std::collections::HashSet<String>,
) -> Result<()> {
    let remote_path = join_remote(remote_root, &local.rel_path);
    crate::utils::retry::with_retries(options.retries, options.retry_backoff_secs, || async {
        crate::services::delete_service::delete(config, &remote_path, true).await
    })
    .await
    .ok();
    upload_one(config, local, remote_root, options, ensured).await?;
    match verify_one(config, remote_root, local, options).await? {
        ExistingAction::Unchanged | ExistingAction::UnchangedHashMissing => Ok(()),
        _ => Err(anyhow!("自愈后校验仍不一致: {}", local.rel_path)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn local(rel: &str, size: u64, sha: &str) -> LocalObject {
        LocalObject {
            rel_path: rel.to_string(),
            abs_path: PathBuf::from(rel),
            size,
            expected_sha256: sha.to_string(),
        }
    }

    fn remote(size: i64, hash: Option<&str>, alg: Option<&str>, folder: bool) -> RemoteObject {
        RemoteObject {
            file_id: "id".to_string(),
            size,
            content_hash: hash.map(|value| value.to_string()),
            content_hash_algorithm: alg.map(|value| value.to_string()),
            is_folder: folder,
        }
    }

    /// 64 位十六进制文件名（内容寻址对象的形状）。
    fn sha(c: char) -> String {
        c.to_string().repeat(64)
    }

    /// 大小写混合的 64 位十六进制。
    fn sha_mixed_case() -> String {
        format!("{}{}", "A".repeat(32), "b".repeat(32))
    }

    #[test]
    fn hash_compare_is_case_insensitive() {
        assert!(hash_matches("ABCDEF", "abcdef"));
        assert!(hash_matches(" abc ", "ABC"));
        assert!(!hash_matches("abc", "abd"));
    }

    /// 分类只看文件名：64 位十六进制即内容寻址，大小写都算，其余一律普通文件。
    #[test]
    fn content_addressed_is_64_hex_file_name() {
        assert!(is_content_addressed(&format!("data/aa/{}", sha('a'))));
        assert!(is_content_addressed(&sha('A')));
        assert!(is_content_addressed(&sha_mixed_case()));
        assert!(is_content_addressed(&format!("index/{}", sha('0'))));
        // 63 / 65 位、非 hex、普通名字都不是内容寻址
        assert!(!is_content_addressed(&sha('a')[..63]));
        assert!(!is_content_addressed(&format!("{}{}", sha('a'), "a")));
        assert!(!is_content_addressed(&"g".repeat(64)));
        assert!(!is_content_addressed("config"));
        assert!(!is_content_addressed("data/aa/config"));
    }

    /// 内容寻址对象用文件名当期望哈希（统一小写），不需要读文件。
    #[test]
    fn expected_hash_comes_from_file_name() {
        let name = sha_mixed_case();
        let hash = expected_sha256(&format!("data/ab/{name}"), Path::new("/nonexistent/x"))
            .expect("内容寻址不读文件");
        assert_eq!(hash, name.to_lowercase());
    }

    #[test]
    fn skip_when_size_and_hash_match() {
        let item = local("data/aa/abc", 10, "abcdef");
        let existing = remote(10, Some("ABCDEF"), Some("SHA256"), false);
        assert!(!needs_upload(&item, &existing, true));
        assert_eq!(
            classify_existing(&item, &existing, true),
            ExistingAction::Unchanged
        );
    }

    #[test]
    fn upload_when_size_differs() {
        let item = local("data/aa/abc", 11, "abcdef");
        let existing = remote(10, Some("abcdef"), Some("SHA256"), false);
        assert!(needs_upload(&item, &existing, true));
    }

    #[test]
    fn upload_when_hash_differs() {
        let item = local("data/aa/abc", 10, "abcdef");
        let existing = remote(10, Some("000000"), Some("SHA256"), false);
        assert!(needs_upload(&item, &existing, true));
    }

    /// 内容寻址对象：同名不同内容 = 远端那份坏了，可以删掉重传（自愈）。
    #[test]
    fn content_addressed_conflict_is_reuploaded() {
        let item = local(&format!("data/aa/{}", sha('a')), 10, &sha('a'));
        let existing = remote(10, Some(&sha('b')), Some("SHA256"), false);
        assert_eq!(
            classify_existing(&item, &existing, true),
            ExistingAction::Reupload
        );
        // 尺寸不同同理
        let existing = remote(11, Some(&sha('a')), Some("SHA256"), false);
        assert_eq!(
            classify_existing(&item, &existing, true),
            ExistingAction::Reupload
        );
    }

    /// 普通文件：同名但内容不同 —— 拒绝覆盖，绝不动远端。
    #[test]
    fn regular_file_conflict_is_refused() {
        let item = local("config", 29, "abcdef");
        let existing = remote(29, Some("000000"), Some("SHA256"), false);
        assert_eq!(
            classify_existing(&item, &existing, true),
            ExistingAction::Fail(ERR_CONTENT_DIFFERS.to_string())
        );
        // 大小不同同样拒绝（不能靠覆盖去「修正」别人的仓）
        let existing = remote(38, Some("abcdef"), Some("SHA256"), false);
        assert_eq!(
            classify_existing(&item, &existing, true),
            ExistingAction::Fail(ERR_CONTENT_DIFFERS.to_string())
        );
    }

    /// 缺哈希：内容寻址对象还有「名字=哈希」这层保证；普通文件没有，必须失败。
    #[test]
    fn missing_remote_hash_depends_on_content_addressing() {
        let addressed = local(&format!("data/aa/{}", sha('a')), 10, &sha('a'));
        let existing = remote(10, None, None, false);
        assert_eq!(
            classify_existing(&addressed, &existing, true),
            ExistingAction::UnchangedHashMissing
        );

        let regular = local("config", 10, "abcdef");
        assert_eq!(
            classify_existing(&regular, &existing, true),
            ExistingAction::Fail(ERR_HASH_UNAVAILABLE.to_string())
        );
    }

    #[test]
    fn algorithm_must_be_sha256_for_comparison() {
        let item = local("data/aa/abc", 10, "abcdef");
        // 算法不是 SHA256：无法比对，按不可用处理，不重传
        let other_alg = remote(10, Some("000000"), Some("MD5"), false);
        assert!(!needs_upload(&item, &other_alg, true));
        assert_eq!(
            check_hash(Some("000000"), Some("MD5"), "abcdef"),
            HashCheck::Unavailable
        );
        // 算法大小写不敏感
        assert_eq!(
            check_hash(Some("ABCDEF"), Some("sha256"), "abcdef"),
            HashCheck::Match
        );
    }

    #[test]
    fn upload_when_remote_is_folder() {
        let item = local("data/aa/abc", 10, "abcdef");
        let existing = remote(0, None, None, true);
        assert!(needs_upload(&item, &existing, true));
        // 普通文件遇到同名目录也只能记失败，不能删别人的目录
        assert_eq!(
            classify_existing(&item, &existing, true),
            ExistingAction::Fail(ERR_CONTENT_DIFFERS.to_string())
        );
    }

    #[test]
    fn verify_disabled_only_compares_size() {
        let item = local("data/aa/abc", 10, "abcdef");
        let existing = remote(10, Some("000000"), Some("SHA256"), false);
        assert!(!needs_upload(&item, &existing, false));
        // 关掉校验就不做哈希判断，也不会因为普通文件缺哈希而失败
        assert_eq!(
            classify_existing(&item, &existing, false),
            ExistingAction::Unchanged
        );
    }

    /// 删除集合 = 远端有、本地没有的**文件**；目录不算，`--delete` 关闭时恒为空。
    #[test]
    fn delete_set_is_remote_files_missing_locally() {
        let mut remote_map = BTreeMap::new();
        remote_map.insert("data".to_string(), remote(0, None, None, true));
        remote_map.insert("data/aa".to_string(), remote(0, None, None, true));
        remote_map.insert("data/aa/x".to_string(), remote(1, None, None, false));
        remote_map.insert("index/old".to_string(), remote(2, None, None, false));
        let locals = vec![local("data/aa/x", 1, "aa")];

        // 关闭 --delete：一个都不删
        assert!(plan_delete_set(&remote_map, &locals, false).is_empty());

        // 打开 --delete：只剩远端多出来的那个文件（目录不参与）
        let plan = plan_delete_set(&remote_map, &locals, true);
        assert_eq!(plan, vec!["index/old".to_string()]);
    }

    /// 本地为空时，删除集合就是远端全部文件——所以调用方必须先用 exit 2 拦住它。
    #[test]
    fn delete_set_with_empty_local_covers_everything() {
        let mut remote_map = BTreeMap::new();
        remote_map.insert("data/aa/x".to_string(), remote(1, None, None, false));
        remote_map.insert("config".to_string(), remote(2, None, None, false));
        let plan = plan_delete_set(&remote_map, &[], true);
        assert_eq!(plan.len(), 2);
    }

    #[test]
    fn join_remote_handles_root() {
        assert_eq!(join_remote("/", "config"), "/config");
        assert_eq!(join_remote("/repo", "config"), "/repo/config");
        assert_eq!(join_remote("/repo/", "/config"), "/repo/config");
    }

    #[test]
    fn parent_paths_are_computed() {
        assert_eq!(parent_remote_path("/repo/config"), "/repo");
        assert_eq!(parent_remote_path("/config"), "/");
    }
}
