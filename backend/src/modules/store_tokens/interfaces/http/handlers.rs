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
use crate::modules::store_tokens::application::service::CreateStoreApiTokenRequest;
use crate::modules::store_tokens::domain::entity::{
    CreateStoreApiTokenResult, RevokeStoreApiTokenResult, StoreApiTokenMetadata,
};
use crate::shared::auth::PlatformRole;
use crate::shared::error::AppError;

#[derive(Debug, Serialize)]
pub struct StoreTokenListResponse {
    pub tokens: Vec<StoreApiTokenMetadata>,
}

#[derive(Debug, Serialize)]
pub struct CreateStoreApiTokenResponse {
    pub token: StoreApiTokenMetadata,
    pub plaintext_token: String,
}

pub async fn list_tokens(
    State(state): State<AppState>,
    ctx: Option<Extension<SessionContext>>,
    Path(store_id): Path<Uuid>,
) -> Result<Json<StoreTokenListResponse>, AppError> {
    let actor = resolve_actor(&state, ctx).await?;
    let tokens = state
        .store_token_service
        .list_tokens(store_id, &actor)
        .await?;

    Ok(Json(StoreTokenListResponse { tokens }))
}

pub async fn create_token(
    State(state): State<AppState>,
    ctx: Option<Extension<SessionContext>>,
    Path(store_id): Path<Uuid>,
    Json(payload): Json<CreateStoreApiTokenRequest>,
) -> Result<(StatusCode, Json<CreateStoreApiTokenResponse>), AppError> {
    let actor = resolve_actor(&state, ctx).await?;
    let CreateStoreApiTokenResult {
        plaintext_token,
        token,
    } = state
        .store_token_service
        .create_token(store_id, payload, &actor)
        .await?;

    Ok((
        StatusCode::CREATED,
        Json(CreateStoreApiTokenResponse {
            token,
            plaintext_token,
        }),
    ))
}

pub async fn revoke_token(
    State(state): State<AppState>,
    ctx: Option<Extension<SessionContext>>,
    Path((store_id, token_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    let actor = resolve_actor(&state, ctx).await?;
    let result = state
        .store_token_service
        .revoke_token(store_id, token_id, &actor)
        .await?;

    match result {
        RevokeStoreApiTokenResult::Revoked | RevokeStoreApiTokenResult::AlreadyRevoked => {
            Ok(StatusCode::NO_CONTENT)
        }
    }
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
