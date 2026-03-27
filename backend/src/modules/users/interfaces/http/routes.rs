use crate::bootstrap::state::AppState;
use crate::modules::users::interfaces::http::handlers;
use axum::{
    routing::{get, post},
    Router,
};

pub fn routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", get(handlers::list_users).post(handlers::create_user))
        .route(
            "/:userId",
            get(handlers::get_user).put(handlers::update_user),
        )
        .route("/:userId/disable", post(handlers::disable_user))
        .with_state(state)
}
