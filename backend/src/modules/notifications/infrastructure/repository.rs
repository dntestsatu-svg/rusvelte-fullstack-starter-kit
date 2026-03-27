use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

use crate::infrastructure::db::DbPool;
use crate::modules::notifications::domain::entity::{
    NotificationStatus, UserNotification,
};
use crate::modules::notifications::domain::repository::NotificationRepository;

pub struct SqlxNotificationRepository {
    db: DbPool,
}

impl SqlxNotificationRepository {
    pub fn new(db: DbPool) -> Self {
        Self { db }
    }
}

#[async_trait]
impl NotificationRepository for SqlxNotificationRepository {
    async fn list_for_user(
        &self,
        user_id: Uuid,
        limit: i64,
        offset: i64,
        status: Option<NotificationStatus>,
    ) -> anyhow::Result<Vec<UserNotification>> {
        let status = status.map(|value| value.to_string());
        let rows = sqlx::query_as::<_, NotificationRow>(
            r#"
            SELECT
                id,
                user_id,
                type,
                title,
                body,
                related_type,
                related_id,
                status,
                created_at
            FROM user_notifications
            WHERE user_id = $1
              AND ($2::TEXT IS NULL OR status = $2)
            ORDER BY created_at DESC
            LIMIT $3 OFFSET $4
            "#,
        )
        .bind(user_id)
        .bind(status)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.db)
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn count_for_user(
        &self,
        user_id: Uuid,
        status: Option<NotificationStatus>,
    ) -> anyhow::Result<i64> {
        let status = status.map(|value| value.to_string());
        let count = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(id)
            FROM user_notifications
            WHERE user_id = $1
              AND ($2::TEXT IS NULL OR status = $2)
            "#,
        )
        .bind(user_id)
        .bind(status)
        .fetch_one(&self.db)
        .await?;

        Ok(count)
    }

    async fn count_unread_for_user(&self, user_id: Uuid) -> anyhow::Result<i64> {
        let count = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(id)
            FROM user_notifications
            WHERE user_id = $1
              AND status = 'unread'
            "#,
        )
        .bind(user_id)
        .fetch_one(&self.db)
        .await?;

        Ok(count)
    }

    async fn find_for_user(
        &self,
        user_id: Uuid,
        notification_id: Uuid,
    ) -> anyhow::Result<Option<UserNotification>> {
        let row = sqlx::query_as::<_, NotificationRow>(
            r#"
            SELECT
                id,
                user_id,
                type,
                title,
                body,
                related_type,
                related_id,
                status,
                created_at
            FROM user_notifications
            WHERE user_id = $1
              AND id = $2
            "#,
        )
        .bind(user_id)
        .bind(notification_id)
        .fetch_optional(&self.db)
        .await?;

        Ok(row.map(Into::into))
    }

    async fn mark_read(&self, user_id: Uuid, notification_id: Uuid) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            UPDATE user_notifications
            SET status = 'read'
            WHERE user_id = $1
              AND id = $2
            "#,
        )
        .bind(user_id)
        .bind(notification_id)
        .execute(&self.db)
        .await?;

        Ok(())
    }
}

#[derive(Debug, FromRow)]
struct NotificationRow {
    id: Uuid,
    user_id: Uuid,
    #[sqlx(rename = "type")]
    notification_type: String,
    title: String,
    body: String,
    related_type: Option<String>,
    related_id: Option<Uuid>,
    status: String,
    created_at: DateTime<Utc>,
}

impl From<NotificationRow> for UserNotification {
    fn from(value: NotificationRow) -> Self {
        Self {
            id: value.id,
            user_id: value.user_id,
            notification_type: value.notification_type,
            title: value.title,
            body: value.body,
            related_type: value.related_type,
            related_id: value.related_id,
            status: NotificationStatus::from(value.status),
            created_at: value.created_at,
        }
    }
}
