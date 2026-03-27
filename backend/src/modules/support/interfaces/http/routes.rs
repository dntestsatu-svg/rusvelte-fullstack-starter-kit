use crate::modules::auth::application::dto::SessionContext;
use crate::modules::support::application::service::{
    ReplyRequest, StatusUpdateRequest, SubmitContactRequest, SupportService,
};
use crate::shared::pagination::PaginationParams;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, patch, post},
    Json, Router,
};
use std::sync::Arc;
use uuid::Uuid;

pub fn support_routes(
    state: crate::bootstrap::state::AppState,
) -> Router<crate::bootstrap::state::AppState> {
    Router::new()
        .route("/inbox", get(list_threads))
        .route("/inbox/:id", get(get_thread_detail))
        .route("/inbox/:id/reply", post(reply_to_thread))
        .route("/inbox/:id/status", patch(update_status))
        .with_state(state)
}

pub fn public_support_routes(
    state: crate::bootstrap::state::AppState,
) -> Router<crate::bootstrap::state::AppState> {
    Router::new()
        .route("/contact", post(submit_contact))
        .with_state(state)
}

async fn submit_contact(
    State(service): State<Arc<SupportService>>,
    Json(req): Json<SubmitContactRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    service
        .submit_contact(req)
        .await
        .map(|_| StatusCode::CREATED)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))
}

async fn list_threads(
    State(service): State<Arc<SupportService>>,
    ctx: SessionContext,
    Query(params): Query<PaginationParams>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // Role check: dev + superadmin only
    if ctx.user.role != "dev" && ctx.user.role != "superadmin" {
        return Err((StatusCode::FORBIDDEN, "Access denied".to_string()));
    }

    let params = params.normalize();
    let limit = params.per_page as i64;
    let offset = params.offset() as i64;

    let threads = service
        .list_threads(limit, offset)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::json!({
        "data": threads,
        "pagination": {
            "page": params.page,
            "per_page": params.per_page
        }
    })))
}

async fn get_thread_detail(
    State(service): State<Arc<SupportService>>,
    ctx: SessionContext,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    if ctx.user.role != "dev" && ctx.user.role != "superadmin" {
        return Err((StatusCode::FORBIDDEN, "Access denied".to_string()));
    }

    let detail = service
        .get_thread_detail(id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Thread not found".to_string()))?;

    Ok(Json(serde_json::json!(detail)))
}

async fn reply_to_thread(
    State(service): State<Arc<SupportService>>,
    ctx: SessionContext,
    Path(id): Path<Uuid>,
    Json(req): Json<ReplyRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    if ctx.user.role != "dev" && ctx.user.role != "superadmin" {
        return Err((StatusCode::FORBIDDEN, "Access denied".to_string()));
    }

    service
        .reply_to_thread(id, ctx.user.id, req)
        .await
        .map(|_| StatusCode::CREATED)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))
}

async fn update_status(
    State(service): State<Arc<SupportService>>,
    ctx: SessionContext,
    Path(id): Path<Uuid>,
    Json(req): Json<StatusUpdateRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    if ctx.user.role != "dev" && ctx.user.role != "superadmin" {
        return Err((StatusCode::FORBIDDEN, "Access denied".to_string()));
    }

    service
        .update_thread_status(id, req)
        .await
        .map(|_| StatusCode::OK)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))
}
