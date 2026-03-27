use axum::{
    extract::{Path, State},
    Extension, Json,
};
use serde::Serialize;
use std::str::FromStr;
use uuid::Uuid;

use crate::bootstrap::state::AppState;
use crate::modules::auth::application::dto::SessionContext;
use crate::modules::payments::application::provider::ProviderBalanceSnapshot;
use crate::shared::auth::{has_capability, Capability, PlatformRole};
use crate::shared::error::AppError;

#[derive(Debug, Serialize)]
pub struct StoreBalanceResponse {
    pub balance: crate::modules::balances::domain::entity::StoreBalanceSnapshot,
}

#[derive(Debug, Serialize)]
pub struct ProviderBalanceResponse {
    pub provider_pending_balance: i64,
    pub provider_settle_balance: i64,
}

pub async fn get_store_balance(
    State(state): State<AppState>,
    ctx: Option<Extension<SessionContext>>,
    Path(store_id): Path<Uuid>,
) -> Result<Json<StoreBalanceResponse>, AppError> {
    let actor = resolve_actor(&state, ctx).await?;
    let balance = state
        .balance_service
        .get_store_balance_snapshot(store_id, &actor)
        .await?
        .ok_or_else(|| AppError::NotFound("Store balance not found".into()))?;

    Ok(Json(StoreBalanceResponse { balance }))
}

pub async fn get_provider_balance(
    State(state): State<AppState>,
    ctx: Option<Extension<SessionContext>>,
) -> Result<Json<ProviderBalanceResponse>, AppError> {
    let actor = resolve_actor(&state, ctx).await?;
    if !has_capability(&actor, Capability::ProviderMonitorRead, None) {
        return Err(AppError::Forbidden(
            "You do not have permission to view provider balances".into(),
        ));
    }

    let ProviderBalanceSnapshot {
        provider_pending_balance,
        provider_settle_balance,
    } = state.payment_service.get_provider_balance_snapshot().await?;

    Ok(Json(ProviderBalanceResponse {
        provider_pending_balance,
        provider_settle_balance,
    }))
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
