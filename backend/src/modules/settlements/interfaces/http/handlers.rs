use axum::{extract::State, Extension, Json};
use serde::Serialize;
use std::str::FromStr;

use crate::bootstrap::state::AppState;
use crate::modules::auth::application::dto::SessionContext;
use crate::modules::settlements::domain::entity::{ProcessedSettlement, SettlementRequest};
use crate::shared::auth::PlatformRole;
use crate::shared::error::AppError;

#[derive(Debug, Serialize)]
pub struct SettlementResponse {
    pub settlement: crate::modules::settlements::domain::entity::SettlementRecord,
    pub balance: crate::modules::balances::domain::entity::StoreBalanceSnapshot,
}

pub async fn create_settlement(
    State(state): State<AppState>,
    ctx: Option<Extension<SessionContext>>,
    Json(request): Json<SettlementRequest>,
) -> Result<Json<SettlementResponse>, AppError> {
    let actor = resolve_actor(&state, ctx).await?;
    let ProcessedSettlement {
        settlement,
        balance,
        notification_user_ids,
    } = state
        .settlement_service
        .process_settlement(request, &actor)
        .await?;

    state.realtime_service.publish_store_balance_updated(
        balance.store_id,
        settlement.id,
        balance.pending_balance,
        balance.settled_balance,
        balance.withdrawable_balance,
    );

    if !notification_user_ids.is_empty() {
        state.realtime_service.publish_notification_created(
            notification_user_ids,
            Some("settlement"),
            Some(settlement.id),
        );
    }

    Ok(Json(SettlementResponse { settlement, balance }))
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
