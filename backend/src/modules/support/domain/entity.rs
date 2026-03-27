use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(rename_all = "snake_case")]
pub enum ThreadStatus {
    #[serde(rename = "open")]
    Open,
    #[serde(rename = "in_progress")]
    InProgress,
    #[serde(rename = "closed")]
    Closed,
    #[serde(rename = "spam")]
    Spam,
}

impl ToString for ThreadStatus {
    fn to_string(&self) -> String {
        match self {
            ThreadStatus::Open => "open".to_string(),
            ThreadStatus::InProgress => "in_progress".to_string(),
            ThreadStatus::Closed => "closed".to_string(),
            ThreadStatus::Spam => "spam".to_string(),
        }
    }
}

impl From<String> for ThreadStatus {
    fn from(s: String) -> Self {
        match s.as_str() {
            "open" => ThreadStatus::Open,
            "in_progress" => ThreadStatus::InProgress,
            "closed" => ThreadStatus::Closed,
            "spam" => ThreadStatus::Spam,
            _ => ThreadStatus::Open,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ThreadStatus;

    #[test]
    fn thread_status_uses_snake_case_values() {
        assert_eq!(ThreadStatus::Open.to_string(), "open");
        assert_eq!(ThreadStatus::InProgress.to_string(), "in_progress");
        assert_eq!(ThreadStatus::Closed.to_string(), "closed");
        assert_eq!(ThreadStatus::Spam.to_string(), "spam");
        assert_eq!(ThreadStatus::from("in_progress".to_string()), ThreadStatus::InProgress);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactThread {
    pub id: Uuid,
    pub name: String,
    pub email: String,
    pub phone: Option<String>,
    pub company_name: Option<String>,
    pub subject: String,
    pub category: String,
    pub status: ThreadStatus,
    pub assigned_to_user_id: Option<Uuid>,
    pub last_message_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(rename_all = "lowercase")]
pub enum SenderType {
    #[serde(rename = "guest")]
    Guest,
    #[serde(rename = "staff")]
    Staff,
}

impl ToString for SenderType {
    fn to_string(&self) -> String {
        match self {
            SenderType::Guest => "guest".to_string(),
            SenderType::Staff => "staff".to_string(),
        }
    }
}

impl From<String> for SenderType {
    fn from(s: String) -> Self {
        match s.as_str() {
            "guest" => SenderType::Guest,
            "staff" => SenderType::Staff,
            _ => SenderType::Guest,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactMessage {
    pub id: Uuid,
    pub thread_id: Uuid,
    pub sender_type: SenderType,
    pub sender_user_id: Option<Uuid>,
    pub body: String,
    pub created_at: DateTime<Utc>,
}
