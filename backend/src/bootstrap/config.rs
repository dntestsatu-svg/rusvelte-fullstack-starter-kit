use crate::shared::error::AppError;
use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub port: u16,
    pub database_url: String,
    pub redis_url: String,
    pub log_level: String,
    pub store_bank_account_encryption_key: String,
    pub external_api_url: String,
    pub external_api_uuid: String,
    pub external_api_client: String,
    pub external_api_secret: String,
    pub external_api_timeout_seconds: u64,
}

impl Config {
    pub fn from_env() -> Result<Self, AppError> {
        // Load .env from workspace root stably
        if let Ok(manifest_dir) = env::var("CARGO_MANIFEST_DIR") {
            let root_env = std::path::PathBuf::from(manifest_dir)
                .parent()
                .map(|p| p.join(".env"));

            if let Some(path) = root_env {
                dotenvy::from_path(path).ok();
            }
        } else {
            dotenvy::dotenv().ok();
        }

        Ok(Self {
            port: get_env("PORT")?
                .parse()
                .map_err(|_| AppError::Config("PORT must be a number".into()))?,
            database_url: get_env("DATABASE_URL")?,
            redis_url: get_env("REDIS_URL")?,
            log_level: env::var("LOG_LEVEL").unwrap_or_else(|_| "info".into()),
            store_bank_account_encryption_key: get_env("STORE_BANK_ACCOUNT_ENCRYPTION_KEY")?,
            external_api_url: get_env("EXTERNAL_API_URL")?,
            external_api_uuid: get_env("EXTERNAL_API_UUID")?,
            external_api_client: get_env("EXTERNAL_API_CLIENT")?,
            external_api_secret: get_env("EXTERNAL_API_SECRET")?,
            external_api_timeout_seconds: env::var("EXTERNAL_API_TIMEOUT_SECONDS")
                .unwrap_or_else(|_| "5".into())
                .parse()
                .map_err(|_| {
                    AppError::Config(
                        "EXTERNAL_API_TIMEOUT_SECONDS must be a positive integer".into(),
                    )
                })?,
        })
    }
}

fn get_env(key: &str) -> Result<String, AppError> {
    env::var(key).map_err(|_| AppError::Config(format!("Environment variable {} is not set", key)))
}
