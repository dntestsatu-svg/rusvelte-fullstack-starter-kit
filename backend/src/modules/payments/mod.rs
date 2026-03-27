pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;

use axum::Router;

use crate::bootstrap::state::AppState;

pub fn client_routes(state: AppState) -> Router<AppState> {
    interfaces::http::routes::client_routes(state)
}

pub fn dashboard_routes(state: AppState) -> Router<AppState> {
    interfaces::http::routes::dashboard_routes(state)
}

pub fn webhook_routes(state: AppState) -> Router<AppState> {
    interfaces::http::routes::webhook_routes(state)
}
