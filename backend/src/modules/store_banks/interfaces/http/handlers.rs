use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};
use serde::Serialize;
use std::str::FromStr;
use uuid::Uuid;

use crate::bootstrap::state::AppState;
use crate::modules::auth::application::dto::SessionContext;
use crate::modules::store_banks::application::service::{
    BankAccountInquiryRequest, CreateStoreBankAccountRequest,
};
use crate::modules::store_banks::domain::entity::{StoreBankAccountSummary, StoreBankInquiry};
use crate::shared::auth::PlatformRole;
use crate::shared::error::AppError;

#[derive(Debug, Serialize)]
pub struct StoreBankListResponse {
    pub banks: Vec<StoreBankAccountSummary>,
}

#[derive(Debug, Serialize)]
pub struct StoreBankInquiryResponse {
    pub inquiry: StoreBankInquiry,
}

#[derive(Debug, Serialize)]
pub struct StoreBankAccountResponse {
    pub bank: StoreBankAccountSummary,
}

pub async fn list_banks(
    State(state): State<AppState>,
    ctx: Option<Extension<SessionContext>>,
    Path(store_id): Path<Uuid>,
) -> Result<Json<StoreBankListResponse>, AppError> {
    let actor = resolve_actor(&state, ctx).await?;
    let banks = state.store_bank_service.list_accounts(store_id, &actor).await?;
    Ok(Json(StoreBankListResponse { banks }))
}

pub async fn inquiry_bank(
    State(state): State<AppState>,
    ctx: Option<Extension<SessionContext>>,
    Path(store_id): Path<Uuid>,
    Json(payload): Json<BankAccountInquiryRequest>,
) -> Result<Json<StoreBankInquiryResponse>, AppError> {
    let actor = resolve_actor(&state, ctx).await?;
    let inquiry = state
        .store_bank_service
        .inquiry_account(store_id, payload, &actor)
        .await?;
    Ok(Json(StoreBankInquiryResponse { inquiry }))
}

pub async fn create_bank(
    State(state): State<AppState>,
    ctx: Option<Extension<SessionContext>>,
    Path(store_id): Path<Uuid>,
    Json(payload): Json<CreateStoreBankAccountRequest>,
) -> Result<(StatusCode, Json<StoreBankAccountResponse>), AppError> {
    let actor = resolve_actor(&state, ctx).await?;
    let bank = state
        .store_bank_service
        .create_account(store_id, payload, &actor)
        .await?;
    Ok((StatusCode::CREATED, Json(StoreBankAccountResponse { bank })))
}

pub async fn set_default_bank(
    State(state): State<AppState>,
    ctx: Option<Extension<SessionContext>>,
    Path((store_id, bank_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<StoreBankAccountResponse>, AppError> {
    let actor = resolve_actor(&state, ctx).await?;
    let bank = state
        .store_bank_service
        .set_default_account(store_id, bank_id, &actor)
        .await?;
    Ok(Json(StoreBankAccountResponse { bank }))
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
