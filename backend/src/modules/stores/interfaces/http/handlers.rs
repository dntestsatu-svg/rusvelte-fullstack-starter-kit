use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Extension, Json,
};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use uuid::Uuid;

use crate::bootstrap::state::AppState;
use crate::modules::auth::application::dto::SessionContext;
use crate::modules::stores::application::service::{
    AddStoreMemberRequest, CreateStoreRequest, StoreListFilters, UpdateStoreMemberRequest,
    UpdateStoreRequest,
};
use crate::modules::stores::domain::entity::{StoreMemberDetail, StoreStatus, StoreSummary};
use crate::shared::auth::{has_capability, Capability, PlatformRole};
use crate::shared::error::AppError;
use crate::shared::pagination::PaginationParams;

#[derive(Debug, Deserialize)]
pub struct StoreListQuery {
    #[serde(flatten)]
    pub pagination: PaginationParams,
    pub search: Option<String>,
    pub status: Option<StoreStatus>,
}

#[derive(Debug, Serialize)]
pub struct StoreListResponse {
    pub stores: Vec<StoreSummary>,
    pub total: i64,
    pub page: u32,
    pub per_page: u32,
}

#[derive(Debug, Serialize)]
pub struct StoreMembersResponse {
    pub members: Vec<StoreMemberDetail>,
}

pub async fn list_stores(
    State(state): State<AppState>,
    ctx: Option<Extension<SessionContext>>,
    Query(query): Query<StoreListQuery>,
) -> Result<Json<StoreListResponse>, AppError> {
    let actor = resolve_actor(&state, ctx).await?;
    ensure_any_capability(
        &actor,
        &[Capability::StoreRead, Capability::StoreReadGlobal],
    )?;

    let params = query.pagination.normalize();
    let filters = StoreListFilters {
        search: query.search,
        status: query.status,
    };

    let stores = state
        .store_service
        .list_stores(
            params.per_page as i64,
            params.offset() as i64,
            filters.clone(),
            &actor,
        )
        .await?;
    let total = state.store_service.count_stores(filters, &actor).await?;

    Ok(Json(StoreListResponse {
        stores,
        total,
        page: params.page,
        per_page: params.per_page,
    }))
}

pub async fn get_store(
    State(state): State<AppState>,
    ctx: Option<Extension<SessionContext>>,
    Path(store_id): Path<Uuid>,
) -> Result<Json<StoreSummary>, AppError> {
    let actor = resolve_actor(&state, ctx).await?;
    ensure_any_capability(
        &actor,
        &[Capability::StoreRead, Capability::StoreReadGlobal],
    )?;

    let store = state
        .store_service
        .get_store(store_id, &actor)
        .await?
        .ok_or_else(|| AppError::NotFound("Store not found".into()))?;

    Ok(Json(store))
}

pub async fn create_store(
    State(state): State<AppState>,
    ctx: Option<Extension<SessionContext>>,
    Json(payload): Json<CreateStoreRequest>,
) -> Result<(StatusCode, Json<StoreSummary>), AppError> {
    let actor = resolve_actor(&state, ctx).await?;
    ensure_capability(&actor, Capability::StoreCreate)?;

    let store = state.store_service.create_store(payload, &actor).await?;
    Ok((StatusCode::CREATED, Json(store)))
}

pub async fn update_store(
    State(state): State<AppState>,
    ctx: Option<Extension<SessionContext>>,
    Path(store_id): Path<Uuid>,
    Json(payload): Json<UpdateStoreRequest>,
) -> Result<Json<StoreSummary>, AppError> {
    let actor = resolve_actor(&state, ctx).await?;

    let store = state
        .store_service
        .update_store(store_id, payload, &actor)
        .await?;
    Ok(Json(store))
}

pub async fn list_members(
    State(state): State<AppState>,
    ctx: Option<Extension<SessionContext>>,
    Path(store_id): Path<Uuid>,
) -> Result<Json<StoreMembersResponse>, AppError> {
    let actor = resolve_actor(&state, ctx).await?;
    let members = state.store_service.list_members(store_id, &actor).await?;
    Ok(Json(StoreMembersResponse { members }))
}

pub async fn add_member(
    State(state): State<AppState>,
    ctx: Option<Extension<SessionContext>>,
    Path(store_id): Path<Uuid>,
    Json(payload): Json<AddStoreMemberRequest>,
) -> Result<(StatusCode, Json<StoreMemberDetail>), AppError> {
    let actor = resolve_actor(&state, ctx).await?;
    let member = state
        .store_service
        .add_member(store_id, payload, &actor)
        .await?;
    Ok((StatusCode::CREATED, Json(member)))
}

pub async fn update_member(
    State(state): State<AppState>,
    ctx: Option<Extension<SessionContext>>,
    Path((store_id, member_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<UpdateStoreMemberRequest>,
) -> Result<Json<StoreMemberDetail>, AppError> {
    let actor = resolve_actor(&state, ctx).await?;
    let member = state
        .store_service
        .update_member(store_id, member_id, payload, &actor)
        .await?;
    Ok(Json(member))
}

pub async fn remove_member(
    State(state): State<AppState>,
    ctx: Option<Extension<SessionContext>>,
    Path((store_id, member_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    let actor = resolve_actor(&state, ctx).await?;
    state
        .store_service
        .remove_member(store_id, member_id, &actor)
        .await?;
    Ok(StatusCode::NO_CONTENT)
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
