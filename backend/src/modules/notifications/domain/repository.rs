use async_trait::async_trait;
use uuid::Uuid;

use crate::modules::notifications::domain::entity::{NotificationStatus, UserNotification};

#[async_trait]
pub trait NotificationRepository: Send + Sync {
    async fn list_for_user(
        &self,
        user_id: Uuid,
        limit: i64,
        offset: i64,
        status: Option<NotificationStatus>,
    ) -> anyhow::Result<Vec<UserNotification>>;

    async fn count_for_user(
        &self,
        user_id: Uuid,
        status: Option<NotificationStatus>,
    ) -> anyhow::Result<i64>;

    async fn count_unread_for_user(&self, user_id: Uuid) -> anyhow::Result<i64>;

    async fn find_for_user(
        &self,
        user_id: Uuid,
        notification_id: Uuid,
    ) -> anyhow::Result<Option<UserNotification>>;

    async fn mark_read(&self, user_id: Uuid, notification_id: Uuid) -> anyhow::Result<()>;
}
