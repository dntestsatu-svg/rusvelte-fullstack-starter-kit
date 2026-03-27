use axum::{
    routing::get,
    Json, Router,
};
use serde_json::{json, Value};
use crate::bootstrap::state::{SharedState, AppState};
use tracing::info;
use std::sync::Arc;
use axum::extract::{FromRef, State};

use crate::modules::auth::interfaces::http::routes::auth_routes;
use crate::modules::auth::interfaces::http::middlewares::{csrf_middleware, rate_limit_middleware};
use axum::middleware as ax_middleware;
use crate::modules::auth::application::service::AuthService;

impl FromRef<AppState> for Arc<AuthService> {
    fn from_ref(state: &AppState) -> Self {
        state.auth_service.clone()
    }
}

pub fn create_router(state: SharedState) -> Router {
    info!("Creating application router...");
    
    let inner_state = Arc::unwrap_or_clone(state);
    
    let api_routes = Router::new()
        .nest("/auth", auth_routes(inner_state.clone()))
        .layer(ax_middleware::from_fn_with_state(inner_state.clone(), rate_limit_middleware))
        .layer(ax_middleware::from_fn_with_state(inner_state.clone(), csrf_middleware));

    Router::new()
        .nest("/api/v1", api_routes)
        .route("/api/v1/health", get(health_check))
        .with_state(inner_state)
}

async fn health_check() -> Json<Value> {
    info!("Health check requested");
    Json(json!({ "status": "ok" }))
}
