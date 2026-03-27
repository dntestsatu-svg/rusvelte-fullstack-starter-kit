use crate::bootstrap::state::AppState;
use crate::modules::auth::interfaces::http::handlers;
use crate::modules::auth::interfaces::http::middlewares::session_middleware;
use axum::{
    middleware,
    routing::{get, post},
    Router,
};

pub fn auth_routes(state: AppState) -> Router<AppState> {
    let authenticated_routes = Router::new()
        .route("/me", get(handlers::me))
        .route("/csrf", get(handlers::get_csrf))
        .route("/logout", post(handlers::logout))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            session_middleware,
        ));

    Router::new()
        .route("/login", post(handlers::login))
        .nest("/", authenticated_routes)
        .with_state(state)
}
