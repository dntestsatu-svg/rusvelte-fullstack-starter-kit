use axum::{routing::get, Router};

use crate::bootstrap::state::AppState;
use crate::modules::balances::interfaces::http::routes as balance_routes;
use crate::modules::payouts::interfaces::http::routes as payout_routes;
use crate::modules::store_banks::interfaces::http::routes as store_bank_routes;
use crate::modules::store_tokens::interfaces::http::routes as store_token_routes;
use crate::modules::stores::interfaces::http::handlers;

pub fn routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", get(handlers::list_stores).post(handlers::create_store))
        .route(
            "/:storeId",
            get(handlers::get_store).put(handlers::update_store),
        )
        .route(
            "/:storeId/members",
            get(handlers::list_members).post(handlers::add_member),
        )
        .route(
            "/:storeId/members/:memberId",
            axum::routing::put(handlers::update_member).delete(handlers::remove_member),
        )
        .merge(balance_routes::store_routes(state.clone()))
        .merge(payout_routes::routes())
        .merge(store_bank_routes::routes())
        .merge(store_token_routes::routes())
        .with_state(state)
}

