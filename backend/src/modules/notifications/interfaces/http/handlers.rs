use std::str::FromStr;

use axum::{
    extract::{Path, Query, State},
    Extension, Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::bootstrap::state::AppState;
use crate::modules::auth::application::dto::SessionContext;
use crate::modules::notifications::application::service::{
    NotificationListFilters, NotificationListResult,
};
use crate::modules::notifications::domain::entity::{NotificationStatus, UserNotification};
use crate::shared::auth::PlatformRole;
use crate::shared::error::AppError;
use crate::shared::pagination::PaginationParams;

#[derive(Debug, Deserialize)]
pub struct NotificationListQuery {
    #[serde(flatten)]
    pub pagination: PaginationParams,
    pub status: Option<NotificationStatus>,
}

#[derive(Debug, Serialize)]
pub struct NotificationListResponse {
    pub notifications: Vec<UserNotification>,
    pub total: i64,
    pub unread_count: i64,
    pub page: u32,
    pub per_page: u32,
}

pub async fn list_notifications(
    State(state): State<AppState>,
    ctx: Option<Extension<SessionContext>>,
    Query(query): Query<NotificationListQuery>,
) -> Result<Json<NotificationListResponse>, AppError> {
    let session = session_from_context(ctx)?;
    let actor = state
        .user_service
        .build_actor(session.user.id, parse_platform_role(&session.user.role)?)
        .await?;
    let params = query.pagination.normalize();
    let NotificationListResult {
        notifications,
        total,
        unread_count,
    } = state
        .notification_service
        .list_notifications(
            session.user.id,
            params.per_page as i64,
            params.offset() as i64,
            NotificationListFilters {
                status: query.status,
            },
            &actor,
        )
        .await?;

    Ok(Json(NotificationListResponse {
        notifications,
        total,
        unread_count,
        page: params.page,
        per_page: params.per_page,
    }))
}

pub async fn mark_notification_read(
    State(state): State<AppState>,
    ctx: Option<Extension<SessionContext>>,
    Path(notification_id): Path<Uuid>,
) -> Result<axum::http::StatusCode, AppError> {
    let session = session_from_context(ctx)?;
    let actor = state
        .user_service
        .build_actor(session.user.id, parse_platform_role(&session.user.role)?)
        .await?;

    state
        .notification_service
        .mark_read(session.user.id, notification_id, &actor)
        .await?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

fn session_from_context(
    ctx: Option<Extension<SessionContext>>,
) -> Result<SessionContext, AppError> {
    ctx.map(|Extension(session)| session)
        .ok_or_else(|| AppError::Unauthorized("Session required".into()))
}

fn parse_platform_role(value: &str) -> Result<PlatformRole, AppError> {
    PlatformRole::from_str(value)
        .map_err(|_| AppError::Unauthorized("Invalid session role".into()))
}
