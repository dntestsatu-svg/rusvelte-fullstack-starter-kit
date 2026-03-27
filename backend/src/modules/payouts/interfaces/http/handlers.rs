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
use crate::modules::payouts::application::service::{PayoutConfirmRequest, PayoutPreviewRequest};
use crate::modules::payouts::domain::entity::{PayoutListRow, PayoutPreviewResult, PayoutRecord};
use crate::shared::auth::PlatformRole;
use crate::shared::error::AppError;

#[derive(Debug, Serialize)]
pub struct PayoutPreviewResponse {
    pub preview: PayoutPreviewResult,
}

#[derive(Debug, Serialize)]
pub struct PayoutResponse {
    pub payout: PayoutRecord,
}

#[derive(Debug, Serialize)]
pub struct PayoutListResponse {
    pub payouts: Vec<PayoutListRow>,
}

#[derive(Debug, Deserialize)]
pub struct PayoutListQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

fn default_limit() -> i64 {
    20
}

pub async fn preview_payout(
    State(state): State<AppState>,
    ctx: Option<Extension<SessionContext>>,
    Path(store_id): Path<Uuid>,
    Json(payload): Json<PayoutPreviewRequest>,
) -> Result<Json<PayoutPreviewResponse>, AppError> {
    let actor = resolve_actor(&state, ctx).await?;
    let preview = state
        .payout_service
        .preview_payout(store_id, payload, &actor)
        .await?;
    Ok(Json(PayoutPreviewResponse { preview }))
}

pub async fn confirm_payout(
    State(state): State<AppState>,
    ctx: Option<Extension<SessionContext>>,
    Path(store_id): Path<Uuid>,
    Json(payload): Json<PayoutConfirmRequest>,
) -> Result<(StatusCode, Json<PayoutResponse>), AppError> {
    let actor = resolve_actor(&state, ctx).await?;
    let payout = state
        .payout_service
        .confirm_payout(store_id, payload, &actor)
        .await?;
    Ok((StatusCode::CREATED, Json(PayoutResponse { payout })))
}

pub async fn list_payouts(
    State(state): State<AppState>,
    ctx: Option<Extension<SessionContext>>,
    Path(store_id): Path<Uuid>,
    Query(query): Query<PayoutListQuery>,
) -> Result<Json<PayoutListResponse>, AppError> {
    let actor = resolve_actor(&state, ctx).await?;
    let payouts = state
        .payout_service
        .list_payouts(store_id, query.limit, query.offset, &actor)
        .await?;
    Ok(Json(PayoutListResponse { payouts }))
}

pub async fn get_payout_detail(
    State(state): State<AppState>,
    ctx: Option<Extension<SessionContext>>,
    Path((store_id, payout_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<PayoutResponse>, AppError> {
    let actor = resolve_actor(&state, ctx).await?;
    let payout = state
        .payout_service
        .get_payout_detail(store_id, payout_id, &actor)
        .await?;
    Ok(Json(PayoutResponse { payout }))
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
