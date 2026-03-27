pub mod application;
pub mod interfaces;

use axum::Router;

use crate::bootstrap::state::AppState;

pub fn routes(state: AppState) -> Router<AppState> {
    interfaces::http::routes::routes(state)
}
