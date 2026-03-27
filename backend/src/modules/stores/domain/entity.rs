use std::fmt::{Display, Formatter};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::shared::auth::StoreRole;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StoreStatus {
    Active,
    Inactive,
}

impl Display for StoreStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            Self::Active => "active",
            Self::Inactive => "inactive",
        };

        f.write_str(value)
    }
}

impl From<String> for StoreStatus {
    fn from(value: String) -> Self {
        match value.as_str() {
            "active" => Self::Active,
            "inactive" => Self::Inactive,
            _ => Self::Inactive,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StoreMemberStatus {
    Active,
    Inactive,
}

impl Display for StoreMemberStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            Self::Active => "active",
            Self::Inactive => "inactive",
        };

        f.write_str(value)
    }
}

impl From<String> for StoreMemberStatus {
    fn from(value: String) -> Self {
        match value.as_str() {
            "active" => Self::Active,
            "inactive" => Self::Inactive,
            _ => Self::Inactive,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Store {
    pub id: Uuid,
    pub owner_user_id: Uuid,
    pub name: String,
    pub slug: String,
    pub status: StoreStatus,
    pub callback_url: Option<String>,
    #[serde(skip_serializing)]
    pub callback_secret: Option<String>,
    pub provider_username: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreSummary {
    pub id: Uuid,
    pub owner_user_id: Uuid,
    pub owner_name: String,
    pub owner_email: String,
    pub name: String,
    pub slug: String,
    pub status: StoreStatus,
    pub callback_url: Option<String>,
    pub provider_username: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreMember {
    pub id: Uuid,
    pub store_id: Uuid,
    pub user_id: Uuid,
    pub store_role: StoreRole,
    pub status: StoreMemberStatus,
    pub invited_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreMemberDetail {
    pub id: Uuid,
    pub store_id: Uuid,
    pub user_id: Uuid,
    pub user_name: String,
    pub user_email: String,
    pub user_platform_role: String,
    pub store_role: StoreRole,
    pub status: StoreMemberStatus,
    pub invited_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
