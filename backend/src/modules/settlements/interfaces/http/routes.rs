use axum::{routing::post, Router};

use crate::bootstrap::state::AppState;
use crate::modules::settlements::interfaces::http::handlers;

pub fn routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/settlements", post(handlers::create_settlement))
        .with_state(state)
}
