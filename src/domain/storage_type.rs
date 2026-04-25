use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum StorageType {
    #[default]
    PersonalNew,
    Family,
    Group,
}

impl StorageType {
    pub fn as_str(&self) -> &'static str {
        match self {
            StorageType::PersonalNew => "personal_new",
            StorageType::Family => "family",
            StorageType::Group => "group",
        }
    }

    pub fn from_str_raw(s: &str) -> Self {
        match s {
            "family" => StorageType::Family,
            "group" => StorageType::Group,
            _ => StorageType::PersonalNew,
        }
    }

    pub fn svc_type(&self) -> &'static str {
        match self {
            StorageType::PersonalNew => "1",
            StorageType::Family => "2",
            StorageType::Group => "3",
        }
    }
}
