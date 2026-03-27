use axum::Router;

use crate::bootstrap::state::AppState;
use crate::modules::payouts::interfaces::http::handlers;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/:storeId/payouts/preview",
            axum::routing::post(handlers::preview_payout),
        )
        .route(
            "/:storeId/payouts",
            axum::routing::get(handlers::list_payouts).post(handlers::confirm_payout),
        )
        .route(
            "/:storeId/payouts/:payoutId",
            axum::routing::get(handlers::get_payout_detail),
        )
}
