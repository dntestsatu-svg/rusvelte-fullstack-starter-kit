use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct AuditLogEntry {
    pub actor_user_id: Option<Uuid>,
    pub action: String,
    pub target_type: Option<String>,
    pub target_id: Option<Uuid>,
    pub payload_json: Value,
}

#[async_trait]
pub trait AuditLogRepository: Send + Sync {
    async fn insert(&self, entry: AuditLogEntry) -> Result<()>;
}
