use axum::{routing::get, Router};

use crate::bootstrap::state::AppState;
use crate::modules::realtime::interfaces::http::handlers;

pub fn routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/stream", get(handlers::stream_events))
        .with_state(state)
}
