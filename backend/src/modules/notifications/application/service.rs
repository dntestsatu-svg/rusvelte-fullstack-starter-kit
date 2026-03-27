use std::sync::Arc;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::modules::notifications::domain::entity::{NotificationStatus, UserNotification};
use crate::modules::notifications::domain::repository::NotificationRepository;
use crate::shared::auth::{has_capability, AuthenticatedUser, Capability};
use crate::shared::error::AppError;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct NotificationListFilters {
    pub status: Option<NotificationStatus>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NotificationListResult {
    pub notifications: Vec<UserNotification>,
    pub total: i64,
    pub unread_count: i64,
}

pub struct NotificationService {
    repository: Arc<dyn NotificationRepository>,
}

impl NotificationService {
    pub fn new(repository: Arc<dyn NotificationRepository>) -> Self {
        Self { repository }
    }

    pub async fn list_notifications(
        &self,
        user_id: Uuid,
        limit: i64,
        offset: i64,
        filters: NotificationListFilters,
        actor: &AuthenticatedUser,
    ) -> Result<NotificationListResult, AppError> {
        ensure_notification_access(actor)?;

        let notifications = self
            .repository
            .list_for_user(user_id, limit, offset, filters.status.clone())
            .await?;
        let total = self
            .repository
            .count_for_user(user_id, filters.status)
            .await?;
        let unread_count = self.repository.count_unread_for_user(user_id).await?;

        Ok(NotificationListResult {
            notifications,
            total,
            unread_count,
        })
    }

    pub async fn mark_read(
        &self,
        user_id: Uuid,
        notification_id: Uuid,
        actor: &AuthenticatedUser,
    ) -> Result<(), AppError> {
        ensure_notification_access(actor)?;

        if self
            .repository
            .find_for_user(user_id, notification_id)
            .await?
            .is_none()
        {
            return Err(AppError::NotFound("Notification not found".into()));
        }

        self.repository.mark_read(user_id, notification_id).await?;
        Ok(())
    }
}

fn ensure_notification_access(actor: &AuthenticatedUser) -> Result<(), AppError> {
    if has_capability(actor, Capability::NotificationRead, None) {
        Ok(())
    } else {
        Err(AppError::Forbidden(
            "You do not have permission to view notifications".into(),
        ))
    }
}
