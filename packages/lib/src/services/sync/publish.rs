use super::paths::{SYNC_UPLOAD_PREFIX, UPLOAD_BACKUP_PREFIX};
use super::remote::{SYNC_LIST_RETRY, remote_sha256};
use super::scan::{CloudSyncFile, CloudSyncScan};
use crate::client::api::personal::client::{ListingRetry, PersonalClient};
use crate::client::http::HttpClientWrapper;
use crate::config::{Config, PersonalConfig};
use crate::error::ClientError;
use crate::models::list::PersonalFileItem;
use crate::progress::Progress;
use crate::protocol::is_non_root_file_id;
use crate::services::directory as dirs;
use crate::services::upload::transfer::transfer_to_parent;
use crate::services::upload::{checked_upload_size, finish_upload};
use crate::utils::{
    crypto::{generate_rand_str, is_sha256_hex},
    remote_path,
};
use anyhow::Context;
use std::path::Path;

pub(super) struct SyncUploadPlan<'a> {
    pub(super) host: &'a str,
    pub(super) scan: Option<&'a CloudSyncScan>,
    pub(super) rel_path: &'a str,
    pub(super) parent_id: &'a str,
    pub(super) remote_parent: &'a str,
    pub(super) file_name: &'a str,
    pub(super) replace_directory_id: Option<&'a str>,
    pub(super) hash_missing: Option<&'a mut bool>,
    pub(super) file_changed: &'a mut bool,
}

/// 先在同级目录上传一个名称唯一的临时文件，再替换目标。
pub(super) async fn upload_sync_to_parent(
    config: PersonalConfig<&Config>,
    local_path: &Path,
    plan: SyncUploadPlan<'_>,
    pb: Option<Progress>,
) -> anyhow::Result<String> {
    let SyncUploadPlan {
        host,
        parent_id,
        remote_parent,
        file_name,
        replace_directory_id,
        hash_missing,
        scan,
        rel_path,
        file_changed,
    } = plan;
    let file_size = checked_upload_size(local_path)?;
    remote_path::validate_name(file_name)?;
    // --verify 发布内容寻址对象（64-hex 文件名）前，先确认本地内容哈希与文件名一致；
    // 不一致说明本地内容已损坏，直接失败并保留远端旧副本（此时还没创建任何远端对象）。
    let verify_content_addressed = hash_missing.is_some() && is_sha256_hex(file_name);
    if verify_content_addressed {
        let path = local_path.to_path_buf();
        let local_hash =
            tokio::task::spawn_blocking(move || crate::utils::crypto::calc_file_sha256(&path))
                .await??;
        anyhow::ensure!(
            local_hash.eq_ignore_ascii_case(file_name),
            "内容寻址对象 {file_name} 的实际内容哈希 {local_hash} 与文件名不一致，已保留远端旧副本"
        );
    }
    let temporary_name = format!("{SYNC_UPLOAD_PREFIX}{}", generate_rand_str(24));
    let client = HttpClientWrapper::new();
    let personal = PersonalClient::with_host(config, host, &client, ListingRetry::None);
    let before = personal.list_files(parent_id).await?;
    if before
        .iter()
        .any(|item| item.name.as_deref() == Some(&temporary_name))
    {
        anyhow::bail!("临时上传名称冲突，旧条目未替换");
    }
    let previous = before
        .iter()
        .find(|item| item.name.as_deref() == Some(file_name));
    let expected_id = if let Some(scan) = scan {
        scan.expected_id(rel_path)?
    } else {
        previous
            .map(|item| {
                item.file_id
                    .as_deref()
                    .filter(|id| is_non_root_file_id(id))
                    .context("旧条目缺少文件 ID")
            })
            .transpose()?
    };
    let expected_file = scan.and_then(|scan| scan.file(rel_path));
    check_destination(&before, file_name, expected_id, expected_file)?;
    anyhow::ensure!(
        replace_directory_id.is_none() || scan.is_some(),
        "替换目录需要扫描记录"
    );
    let uploaded = transfer_to_parent(
        &personal,
        local_path,
        parent_id,
        &temporary_name,
        file_size,
        pb.clone(),
    )
    .await
    .with_context(|| {
        format!("同步上传 {file_name} 失败，旧条目未替换；临时名称 {temporary_name}")
    })?;
    let saved_name = uploaded.name.as_deref().unwrap_or(&temporary_name);
    // 本地源文件可能在前置校验后变化。发布前再次校验上传初始化和远端验证
    // 共用的哈希，确保实际上传内容符合目标文件名。
    if verify_content_addressed {
        anyhow::ensure!(
            uploaded.sha256.eq_ignore_ascii_case(file_name),
            "内容寻址对象 {file_name} 的上传内容哈希 {} 与文件名不一致；临时名称 {saved_name}，旧条目未替换",
            uploaded.sha256
        );
    }
    let promote = async {
        let id = uploaded.id.as_str();
        if before
            .iter()
            .any(|item| item.file_id.as_deref() == Some(id))
        {
            anyhow::bail!("临时文件 ID 已存在，旧条目未替换");
        }
        let mut files = confirm_uploaded_name(&personal, parent_id, id, saved_name).await?;
        check_destination(&files, file_name, expected_id, expected_file)?;
        if let Some(hash_missing) = hash_missing {
            let item = files
                .iter()
                .find(|item| {
                    item.file_id.as_deref() == Some(id) && item.name.as_deref() == Some(saved_name)
                })
                .context("找不到已确认的临时文件")?;
            let remote = CloudSyncFile::from(item);
            if remote.size.is_some_and(|size| size != uploaded.size) {
                anyhow::bail!("上传后校验失败：临时文件大小不一致");
            }
            let digest =
                remote_sha256(config, host, &remote, Some(uploaded.size), hash_missing).await?;
            if digest != uploaded.sha256 {
                anyhow::bail!("上传后校验失败：临时文件 SHA-256 不一致");
            }
            if remote.sha256.is_none() {
                files = confirm_uploaded_name(&personal, parent_id, id, saved_name).await?;
                check_destination(&files, file_name, expected_id, expected_file)?;
            }
        }
        let listing = personal.with_listing_retry(SYNC_LIST_RETRY);
        if let Some(dir_id) = replace_directory_id {
            scan.context("替换目录需要扫描记录")?
                .confirm_subtree(&listing, rel_path, dir_id)
                .await?;
            files = confirm_uploaded_name(&personal, parent_id, id, saved_name).await?;
            check_destination(&files, file_name, expected_id, expected_file)?;
        }
        dirs::confirm_directory(&listing, remote_parent, parent_id).await?;
        let installed = install_uploaded_file(
            &personal,
            parent_id,
            id,
            file_name,
            &files,
            replace_directory_id,
            file_changed,
        )
        .await?;
        finish_upload(&installed, &pb);
        Ok(id.to_owned())
    }
    .await
    .with_context(|| format!("临时名称 {saved_name}，替换 {file_name} 未完成"))?;
    Ok(promote)
}

fn check_destination(
    files: &[PersonalFileItem],
    name: &str,
    expected_id: Option<&str>,
    expected_file: Option<&CloudSyncFile>,
) -> anyhow::Result<()> {
    let mut matches = files
        .iter()
        .filter(|item| item.name.as_deref() == Some(name));
    let unchanged = match (matches.next(), expected_id) {
        (None, None) => true,
        (Some(item), Some(id)) => {
            item.file_id.as_deref() == Some(id)
                && matches.next().is_none()
                && expected_file
                    .is_none_or(|file| !item.is_folder() && CloudSyncFile::from(item) == *file)
        }
        _ => false,
    };
    anyhow::ensure!(unchanged, "目标 {name} 在扫描后发生变化");
    Ok(())
}

async fn install_uploaded_file(
    personal: &PersonalClient<'_>,
    parent_file_id: &str,
    file_id: &str,
    file_name: &str,
    files: &[PersonalFileItem],
    replace_directory_id: Option<&str>,
    file_changed: &mut bool,
) -> Result<String, ClientError> {
    let saved_name = files
        .iter()
        .find(|item| item.file_id.as_deref() == Some(file_id) && !item.is_folder())
        .and_then(|item| item.name.as_deref())
        .filter(|name| !name.is_empty())
        .ok_or_else(|| ClientError::Api("分片上传已提交，但无法确认新文件名称".into()))?;
    if saved_name == file_name {
        *file_changed = true;
        return Ok(saved_name.to_owned());
    }

    let mut conflicts = files
        .iter()
        .filter(|item| item.name.as_deref() == Some(file_name));
    let old_file = conflicts.next();
    if let Some(item) = old_file {
        let old_id = item
            .file_id
            .as_deref()
            .filter(|id| is_non_root_file_id(id) && *id != file_id)
            .ok_or(ClientError::InvalidFilePath)?;
        if conflicts.next().is_some() || item.is_folder().then_some(old_id) != replace_directory_id
        {
            return Err(ClientError::Api(format!(
                "同名条目无法安全替换，新文件保存在 {saved_name}"
            )));
        }
        let backup_name = format!("{UPLOAD_BACKUP_PREFIX}{}", generate_rand_str(20));
        personal
            .rename(old_id, &backup_name)
            .await
            .map_err(|error| {
                ClientError::Api(format!(
                    "无法为旧文件保留备份，新文件保存在 {saved_name}: {error}"
                ))
            })?;
        personal.rename(file_id, file_name).await.map_err(|error| {
            ClientError::Api(format!(
                "无法重命名新文件；旧文件保留为 {backup_name}，请检查新文件 {saved_name}: {error}"
            ))
        })?;
        let published = confirm_uploaded_name(personal, parent_file_id, file_id, file_name)
            .await
            .map_err(|error| {
                ClientError::Api(format!(
                    "无法确认新文件名称；旧文件备份 {backup_name} 未删除: {error}"
                ))
            })?;
        *file_changed = true;
        if !published.iter().any(|entry| {
            entry.name.as_deref() == Some(&backup_name) && entry.file_id.as_deref() == Some(old_id)
        }) {
            return Err(ClientError::Api(format!(
                "旧文件备份 {backup_name} 已变更，停止回收"
            )));
        }
        personal.trash(old_id).await.map_err(|error| {
            ClientError::Api(format!(
                "新文件已重命名为 {file_name}，但旧文件备份 {backup_name} 未确认移到回收站: {error}"
            ))
        })?;
    } else {
        personal.rename(file_id, file_name).await?;
        confirm_uploaded_name(personal, parent_file_id, file_id, file_name).await?;
        *file_changed = true;
    }
    Ok(file_name.to_owned())
}

async fn confirm_uploaded_name(
    personal: &PersonalClient<'_>,
    parent_id: &str,
    file_id: &str,
    name: &str,
) -> Result<Vec<PersonalFileItem>, ClientError> {
    for attempt in 0..5 {
        let files = personal.list_files(parent_id).await?;
        let mut matches = files
            .iter()
            .filter(|item| item.name.as_deref() == Some(name));
        if matches
            .next()
            .is_some_and(|item| item.file_id.as_deref() == Some(file_id) && !item.is_folder())
            && matches.next().is_none()
            && files
                .iter()
                .filter(|item| item.file_id.as_deref() == Some(file_id))
                .count()
                == 1
        {
            return Ok(files);
        }
        if attempt < 4 {
            tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;
        }
    }
    Err(ClientError::Api(format!(
        "无法在云端列表确认新文件 {name} 的身份"
    )))
}

#[cfg(test)]
mod tests {
    use super::install_uploaded_file;
    use crate::client::api::personal::client::{ListingRetry, PersonalClient};
    use crate::client::http::HttpClientWrapper;
    use crate::config::PersonalConfig;
    use httpmock::prelude::*;
    use serde_json::json;

    #[tokio::test]
    async fn acknowledged_but_unconfirmed_rename_never_trashes_backup() {
        let server = MockServer::start();
        let config = crate::config::Config {
            personal_cloud_host: Some(server.base_url()),
            ..Default::default()
        };
        let list = server.mock(|when, then| {
            when.method(POST).path("/file/list");
            then.json_body(json!({"success":true,"data":{"items":[
                {"fileId":"old","name":"report.txt","type":"file"},
                {"fileId":"new","name":"temporary.txt","type":"file"}
            ]}}));
        });
        let rename = server.mock(|when, then| {
            when.path("/file/update");
            then.json_body(json!({"success":true}));
        });
        let trash = server.mock(|when, then| {
            when.path("/recyclebin/batchTrash");
            then.json_body(json!({"success":true}));
        });
        let client = HttpClientWrapper::new();
        let personal = PersonalClient::with_host(
            PersonalConfig::new(&config).unwrap(),
            server.base_url(),
            &client,
            ListingRetry::None,
        );
        let files = personal.list_files("/").await.unwrap();
        let mut file_changed = false;
        let error = install_uploaded_file(
            &personal,
            "/",
            "new",
            "report.txt",
            &files,
            None,
            &mut file_changed,
        )
        .await
        .unwrap_err();
        assert!(!file_changed);
        assert!(error.to_string().contains("未删除"));
        assert!(error.to_string().contains(".cloud139-backup-"));
        list.assert_calls(6);
        rename.assert_calls(2);
        trash.assert_calls(0);
    }

    #[tokio::test]
    async fn failed_upload_conflict_renames_do_not_trash_existing_file() {
        for fail_backup in [true, false] {
            let server = MockServer::start();
            let config = crate::config::Config {
                authorization: "fictional".into(),
                account: "13800138000".into(),
                personal_cloud_host: Some(server.base_url()),
                ..Default::default()
            };
            let listing = server.mock(|when, then| {
                when.method(POST).path("/file/list");
                then.json_body(json!({"success":true,"data":{"items":[
                    {"name":"report.txt","fileId":"old","type":"file"},
                    {"name":"report-copy.txt","fileId":"new","type":"file"}
                ]}}));
            });
            let backup = server.mock(|when, then| {
                when.method(POST)
                    .path("/file/update")
                    .json_body_includes(r#"{"fileId":"old"}"#);
                then.json_body(json!({"success":!fail_backup}));
            });
            let rename = server.mock(|when, then| {
                when.method(POST)
                    .path("/file/update")
                    .json_body_includes(r#"{"fileId":"new","name":"report.txt"}"#);
                then.json_body(json!({"success":false,"message":"rename rejected"}));
            });
            let trash = server.mock(|when, then| {
                when.method(POST).path("/recyclebin/batchTrash");
                then.json_body(json!({"success":true}));
            });
            let client = HttpClientWrapper::new();
            let personal = PersonalClient::with_host(
                PersonalConfig::new(&config).unwrap(),
                server.base_url(),
                &client,
                ListingRetry::None,
            );
            let files = personal.list_files("/").await.unwrap();
            let mut file_changed = false;
            let result = install_uploaded_file(
                &personal,
                "/",
                "new",
                "report.txt",
                &files,
                None,
                &mut file_changed,
            )
            .await;
            assert!(!file_changed);
            let message = result.unwrap_err().to_string();
            assert!(message.contains("report-copy.txt"));
            if !fail_backup {
                assert!(message.contains(".cloud139-backup-"));
            }
            listing.assert_calls(1);
            backup.assert_calls(1);
            rename.assert_calls(usize::from(!fail_backup));
            trash.assert_calls(0);
        }
    }
}
