use axum::{
    routing::{get, post},
    Router,
};

use crate::bootstrap::state::AppState;
use crate::modules::notifications::interfaces::http::handlers;

pub fn routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", get(handlers::list_notifications))
        .route("/:notificationId/read", post(handlers::mark_notification_read))
        .with_state(state)
}
