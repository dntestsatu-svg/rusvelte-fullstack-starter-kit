use crate::bootstrap::state::{AppState, SharedState};
use axum::extract::FromRef;
use axum::{routing::get, Json, Router};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::info;

use crate::modules::auth::application::service::AuthService;
use crate::modules::auth::interfaces::http::middlewares::{
    csrf_middleware, rate_limit_middleware, session_middleware,
};
use crate::modules::auth::interfaces::http::routes::auth_routes;
use crate::modules::stores::application::service::StoreService;
use crate::modules::support::application::service::SupportService;
use crate::modules::support::interfaces::http::routes::{public_support_routes, support_routes};
use axum::middleware as ax_middleware;

impl FromRef<AppState> for Arc<AuthService> {
    fn from_ref(state: &AppState) -> Self {
        state.auth_service.clone()
    }
}

impl FromRef<AppState> for Arc<SupportService> {
    fn from_ref(state: &AppState) -> Self {
        state.support_service.clone()
    }
}

impl FromRef<AppState> for Arc<StoreService> {
    fn from_ref(state: &AppState) -> Self {
        state.store_service.clone()
    }
}

pub fn create_router(state: SharedState) -> Router {
    info!("Creating application router...");

    let inner_state = Arc::unwrap_or_clone(state);

    let auth_api_routes = Router::new()
        .nest("/auth", auth_routes(inner_state.clone()))
        .layer(ax_middleware::from_fn_with_state(
            inner_state.clone(),
            rate_limit_middleware,
        ));

    let protected_api_routes = Router::new()
        .nest("/support", support_routes(inner_state.clone()))
        .nest(
            "/stores",
            crate::modules::stores::routes(inner_state.clone()),
        )
        .nest("/users", crate::modules::users::routes(inner_state.clone()))
        .layer(ax_middleware::from_fn_with_state(
            inner_state.clone(),
            csrf_middleware,
        ))
        .layer(ax_middleware::from_fn_with_state(
            inner_state.clone(),
            session_middleware,
        ))
        .layer(ax_middleware::from_fn_with_state(
            inner_state.clone(),
            rate_limit_middleware,
        ));
    let client_api_routes = Router::new().nest(
        "/client",
        crate::modules::payments::client_routes(inner_state.clone()),
    );

    Router::new()
        .nest(
            "/api/v1",
            Router::new()
                .merge(auth_api_routes)
                .merge(client_api_routes)
                .merge(protected_api_routes),
        )
        .nest(
            "/api/v1/public",
            public_support_routes(inner_state.clone()).layer(ax_middleware::from_fn_with_state(
                inner_state.clone(),
                rate_limit_middleware,
            )),
        )
        .route("/api/v1/health", get(health_check))
        .with_state(inner_state)
}

async fn health_check() -> Json<Value> {
    info!("Health check requested");
    Json(json!({ "status": "ok" }))
}
