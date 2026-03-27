use super::config::Config;
use crate::infrastructure::db::DbPool;
use crate::infrastructure::redis::RedisPool;
use crate::modules::auth::application::service::AuthService;
use crate::modules::balances::application::service::StoreBalanceService;
use crate::modules::notifications::application::service::NotificationService;
use crate::modules::payments::application::idempotency::PaymentIdempotencyService;
use crate::modules::payments::application::service::PaymentService;
use crate::modules::realtime::application::service::RealtimeService;
use crate::modules::settlements::application::service::SettlementService;
use crate::modules::store_banks::application::service::StoreBankService;
use crate::modules::store_tokens::application::service::StoreTokenService;
use crate::modules::stores::application::service::StoreService;
use crate::modules::support::application::service::SupportService;
use crate::modules::users::application::service::UserService;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub db: DbPool,
    pub redis: RedisPool,
    pub auth_service: Arc<AuthService>,
    pub balance_service: Arc<StoreBalanceService>,
    pub notification_service: Arc<NotificationService>,
    pub payment_idempotency_service: Arc<PaymentIdempotencyService>,
    pub payment_service: Arc<PaymentService>,
    pub realtime_service: Arc<RealtimeService>,
    pub settlement_service: Arc<SettlementService>,
    pub store_bank_service: Arc<StoreBankService>,
    pub store_service: Arc<StoreService>,
    pub store_token_service: Arc<StoreTokenService>,
    pub support_service: Arc<SupportService>,
    pub user_service: Arc<UserService>,
}

pub type SharedState = Arc<AppState>;
