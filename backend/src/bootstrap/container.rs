use super::config::Config;
use super::state::AppState;
use crate::infrastructure::db::init_db_pool;
use crate::infrastructure::redis::init_redis_pool;
use crate::shared::error::AppError;
use std::sync::Arc;
use tracing::info;

pub struct Container;

impl Container {
    pub async fn build() -> Result<Arc<AppState>, AppError> {
        info!("Building application container...");

        let config = Config::from_env()?;
        
        let db = init_db_pool(&config.database_url).await?;
        let redis = init_redis_pool(&config.redis_url).await?;

        let state = Arc::new(AppState {
            config,
            db,
            redis,
        });

        info!("Application container built successfully.");
        Ok(state)
    }
}
