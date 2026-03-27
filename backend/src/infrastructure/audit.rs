use async_trait::async_trait;
use sqlx::PgPool;

use crate::shared::audit::{AuditLogEntry, AuditLogRepository};

pub struct SqlxAuditLogRepository {
    pool: PgPool,
}

impl SqlxAuditLogRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AuditLogRepository for SqlxAuditLogRepository {
    async fn insert(&self, entry: AuditLogEntry) -> anyhow::Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO audit_logs (actor_user_id, action, target_type, target_id, payload_json)
            VALUES ($1, $2, $3, $4, $5)
            "#,
            entry.actor_user_id,
            entry.action,
            entry.target_type,
            entry.target_id,
            entry.payload_json
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
