use super::config::Config;
use crate::infrastructure::db::DbPool;
use crate::infrastructure::redis::RedisPool;
use crate::modules::auth::application::service::AuthService;
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
    pub store_service: Arc<StoreService>,
    pub store_token_service: Arc<StoreTokenService>,
    pub support_service: Arc<SupportService>,
    pub user_service: Arc<UserService>,
}

pub type SharedState = Arc<AppState>;
