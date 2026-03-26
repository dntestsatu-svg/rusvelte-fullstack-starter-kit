use sqlx::postgres::PgPoolOptions;
use sqlx::{Pool, Postgres};
use tracing::info;
use crate::shared::error::AppError;

pub type DbPool = Pool<Postgres>;

pub async fn init_db_pool(database_url: &str) -> Result<DbPool, AppError> {
    info!("Initializing database connection pool...");
    
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await
        .map_err(|e| AppError::Database(e))?;

    // Verify connectivity
    sqlx::query("SELECT 1")
        .execute(&pool)
        .await
        .map_err(|e| AppError::Database(e))?;
    
    info!("Database connection pool initialized successfully.");
    Ok(pool)
}
