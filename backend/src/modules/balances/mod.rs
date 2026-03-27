pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;

use crate::bootstrap::state::AppState;
use axum::Router;

pub fn store_routes(state: AppState) -> Router<AppState> {
    interfaces::http::routes::store_routes(state)
}

pub fn dev_routes(state: AppState) -> Router<AppState> {
    interfaces::http::routes::dev_routes(state)
}
