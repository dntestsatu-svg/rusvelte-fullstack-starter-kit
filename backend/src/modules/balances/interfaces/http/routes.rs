use axum::{routing::get, Router};

use crate::bootstrap::state::AppState;
use crate::modules::balances::interfaces::http::handlers;

pub fn store_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/:storeId/balances", get(handlers::get_store_balance))
        .with_state(state)
}

pub fn dev_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/provider/balance", get(handlers::get_provider_balance))
        .with_state(state)
}
