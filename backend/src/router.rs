use axum::{
    routing::get,
    Json, Router,
};
use serde_json::{json, Value};
use crate::bootstrap::state::SharedState;
use tracing::info;

pub fn create_router(state: SharedState) -> Router {
    info!("Creating application router...");
    
    Router::new()
        .route("/api/v1/health", get(health_check))
        .with_state(state)
}

async fn health_check() -> Json<Value> {
    info!("Health check requested");
    Json(json!({ "status": "ok" }))
}
