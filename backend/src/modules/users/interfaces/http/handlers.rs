use axum::{
    extract::{Path, Query, State},
    Extension, Json,
};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use uuid::Uuid;

use crate::bootstrap::state::AppState;
use crate::modules::auth::application::dto::SessionContext;
use crate::modules::users::application::service::{
    CreateUserRequest, UpdateUserRequest, UserListFilters,
};
use crate::modules::users::domain::entity::User;
use crate::shared::auth::{has_capability, Capability, PlatformRole};
use crate::shared::error::AppError;
use crate::shared::pagination::PaginationParams;
use chrono::{DateTime, Utc};

#[derive(Debug, Deserialize)]
pub struct UserListQuery {
    #[serde(flatten)]
    pub pagination: PaginationParams,
    pub role: Option<PlatformRole>,
    pub search: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct UserListResponse {
    pub users: Vec<UserResponse>,
    pub total: i64,
    pub page: u32,
    pub per_page: u32,
}

#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: Uuid,
    pub name: String,
    pub email: String,
    pub role: PlatformRole,
    pub status: crate::modules::users::domain::entity::UserStatus,
    pub created_by: Option<Uuid>,
    pub last_login_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub async fn list_users(
    State(state): State<AppState>,
    ctx: Option<Extension<SessionContext>>,
    Query(query): Query<UserListQuery>,
) -> Result<Json<UserListResponse>, AppError> {
    let actor = resolve_actor(&state, ctx).await?;
    ensure_any_capability(&actor, &[Capability::UserReadGlobal, Capability::UserRead])?;

    let params = query.pagination.normalize();
    let filters = UserListFilters {
        role: query.role,
        search: query.search,
    };

    let users = state
        .user_service
        .list_users(
            params.per_page as i64,
            params.offset() as i64,
            filters.clone(),
            &actor,
        )
        .await?;
    let total = state.user_service.count_users(filters, &actor).await?;

    Ok(Json(UserListResponse {
        users: users.into_iter().map(Into::into).collect(),
        total,
        page: params.page,
        per_page: params.per_page,
    }))
}

pub async fn get_user(
    State(state): State<AppState>,
    ctx: Option<Extension<SessionContext>>,
    Path(id): Path<Uuid>,
) -> Result<Json<UserResponse>, AppError> {
    let actor = resolve_actor(&state, ctx).await?;
    ensure_any_capability(&actor, &[Capability::UserReadGlobal, Capability::UserRead])?;

    let user = state
        .user_service
        .get_user(id, &actor)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".into()))?;

    Ok(Json(user.into()))
}

pub async fn create_user(
    State(state): State<AppState>,
    ctx: Option<Extension<SessionContext>>,
    Json(payload): Json<CreateUserRequest>,
) -> Result<(axum::http::StatusCode, Json<UserResponse>), AppError> {
    let actor = resolve_actor(&state, ctx).await?;
    ensure_capability(&actor, Capability::UserCreate)?;

    let user = state.user_service.create_user(payload, &actor).await?;
    Ok((axum::http::StatusCode::CREATED, Json(user.into())))
}

pub async fn update_user(
    State(state): State<AppState>,
    ctx: Option<Extension<SessionContext>>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateUserRequest>,
) -> Result<Json<UserResponse>, AppError> {
    let actor = resolve_actor(&state, ctx).await?;
    ensure_capability(&actor, Capability::UserUpdate)?;

    let user = state.user_service.update_user(id, payload, &actor).await?;
    Ok(Json(user.into()))
}

pub async fn disable_user(
    State(state): State<AppState>,
    ctx: Option<Extension<SessionContext>>,
    Path(id): Path<Uuid>,
) -> Result<axum::http::StatusCode, AppError> {
    let actor = resolve_actor(&state, ctx).await?;
    ensure_capability(&actor, Capability::UserDisable)?;

    state.user_service.disable_user(id, &actor).await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

async fn resolve_actor(
    state: &AppState,
    ctx: Option<Extension<SessionContext>>,
) -> Result<crate::shared::auth::AuthenticatedUser, AppError> {
    let session = ctx
        .map(|Extension(session)| session)
        .ok_or_else(|| AppError::Unauthorized("Session required".into()))?;

    let platform_role = PlatformRole::from_str(&session.user.role)
        .map_err(|_| AppError::Unauthorized("Invalid session role".into()))?;

    state
        .user_service
        .build_actor(session.user.id, platform_role)
        .await
}

fn ensure_capability(
    actor: &crate::shared::auth::AuthenticatedUser,
    capability: Capability,
) -> Result<(), AppError> {
    if has_capability(actor, capability, None) {
        Ok(())
    } else {
        Err(AppError::Forbidden(
            "You do not have permission to perform this action".into(),
        ))
    }
}

fn ensure_any_capability(
    actor: &crate::shared::auth::AuthenticatedUser,
    capabilities: &[Capability],
) -> Result<(), AppError> {
    if capabilities
        .iter()
        .copied()
        .any(|capability| has_capability(actor, capability, None))
    {
        Ok(())
    } else {
        Err(AppError::Forbidden(
            "You do not have permission to perform this action".into(),
        ))
    }
}

impl From<User> for UserResponse {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            name: user.name,
            email: user.email,
            role: user.role,
            status: user.status,
            created_by: user.created_by,
            last_login_at: user.last_login_at,
            created_at: user.created_at,
            updated_at: user.updated_at,
        }
    }
}
