use super::config::Config;
use crate::infrastructure::db::DbPool;
use crate::infrastructure::redis::RedisPool;
use crate::modules::auth::application::service::AuthService;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub db: DbPool,
    pub redis: RedisPool,
    pub auth_service: Arc<AuthService>,
}

pub type SharedState = Arc<AppState>;
