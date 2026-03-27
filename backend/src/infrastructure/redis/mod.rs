use crate::shared::error::AppError;
use bb8::Pool;
use bb8_redis::RedisConnectionManager;
use tracing::info;

pub type RedisPool = Pool<RedisConnectionManager>;

pub async fn init_redis_pool(redis_url: &str) -> Result<RedisPool, AppError> {
    info!("Initializing Redis connection pool...");

    let manager = RedisConnectionManager::new(redis_url)
        .map_err(|e| AppError::Redis(format!("Invalid Redis URL: {}", e)))?;

    let pool = Pool::builder()
        .max_size(10)
        .build(manager)
        .await
        .map_err(|e| AppError::Redis(format!("Could not build Redis pool: {}", e)))?;

    // Verify connectivity
    {
        let mut conn = pool
            .get()
            .await
            .map_err(|e| AppError::Redis(format!("Could not get Redis connection: {}", e)))?;

        let pong: String = bb8_redis::redis::cmd("PING")
            .query_async(&mut *conn)
            .await
            .map_err(|e| AppError::Redis(format!("Redis PING failed: {}", e)))?;

        if pong != "PONG" {
            return Err(AppError::Redis(format!(
                "Unexpected Redis PING response: {}",
                pong
            )));
        }
    }

    info!("Redis connection pool initialized successfully.");
    Ok(pool)
}
