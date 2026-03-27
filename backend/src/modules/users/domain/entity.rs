use crate::shared::auth::PlatformRole;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UserStatus {
    Active,
    Inactive,
    Suspended,
}

impl From<String> for UserStatus {
    fn from(s: String) -> Self {
        match s.as_str() {
            "active" => Self::Active,
            "inactive" => Self::Inactive,
            "suspended" => Self::Suspended,
            _ => Self::Inactive,
        }
    }
}

impl ToString for UserStatus {
    fn to_string(&self) -> String {
        match self {
            Self::Active => "active".to_string(),
            Self::Inactive => "inactive".to_string(),
            Self::Suspended => "suspended".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub name: String,
    pub email: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub role: PlatformRole,
    pub status: UserStatus,
    pub created_by: Option<Uuid>,
    pub last_login_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}
