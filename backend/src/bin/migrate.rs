use backend::bootstrap::config::Config;
use backend::infrastructure::db::init_db_pool;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    let config = Config::from_env()?;
    let pool = init_db_pool(&config.database_url).await?;

    println!("--- CLEAN ROOM RESET: Option A ---");
    println!("Dropping schema public...");
    sqlx::query("DROP SCHEMA public CASCADE")
        .execute(&pool)
        .await?;

    println!("Creating schema public...");
    sqlx::query("CREATE SCHEMA public").execute(&pool).await?;

    println!("Granting privileges...");
    sqlx::query("GRANT ALL ON SCHEMA public TO public")
        .execute(&pool)
        .await?;

    println!("Running repository migrations...");
    sqlx::migrate!("./migrations").run(&pool).await?;

    println!("--- RESET COMPLETE ---");
    Ok(())
}
