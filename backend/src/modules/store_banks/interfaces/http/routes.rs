use axum::Router;

use crate::bootstrap::state::AppState;
use crate::modules::store_banks::interfaces::http::handlers;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/:storeId/banks",
            axum::routing::get(handlers::list_banks).post(handlers::create_bank),
        )
        .route(
            "/:storeId/banks/inquiry",
            axum::routing::post(handlers::inquiry_bank),
        )
        .route(
            "/:storeId/banks/:bankId/default",
            axum::routing::post(handlers::set_default_bank),
        )
}
