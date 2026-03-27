use std::fmt::{Display, Formatter};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NotificationStatus {
    Unread,
    Read,
}

impl Display for NotificationStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            Self::Unread => "unread",
            Self::Read => "read",
        };

        f.write_str(value)
    }
}

impl From<String> for NotificationStatus {
    fn from(value: String) -> Self {
        match value.as_str() {
            "read" => Self::Read,
            _ => Self::Unread,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserNotification {
    pub id: Uuid,
    pub user_id: Uuid,
    pub notification_type: String,
    pub title: String,
    pub body: String,
    pub related_type: Option<String>,
    pub related_id: Option<Uuid>,
    pub status: NotificationStatus,
    pub created_at: DateTime<Utc>,
}
