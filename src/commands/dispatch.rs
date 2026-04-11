use crate::client::StorageType;
use anyhow::Context;
use std::future::Future;

pub fn load_config() -> anyhow::Result<crate::config::Config> {
    crate::config::Config::load().context("加载配置失败")
}

pub async fn dispatch_to_storage<R, FPersonal, FFamily, FGroup>(
    config: &crate::config::Config,
    personal: FPersonal,
    family: FFamily,
    group: FGroup,
) -> anyhow::Result<R>
where
    FPersonal: Future<Output = anyhow::Result<R>>,
    FFamily: Future<Output = anyhow::Result<R>>,
    FGroup: Future<Output = anyhow::Result<R>>,
{
    match config.storage_type() {
        StorageType::PersonalNew => personal.await,
        StorageType::Family => family.await,
        StorageType::Group => group.await,
    }
}
