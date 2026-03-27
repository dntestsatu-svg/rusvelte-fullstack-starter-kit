use std::time::Duration;

use reqwest::Url;

use crate::bootstrap::config::Config;
use crate::shared::error::AppError;

pub const DEFAULT_PROVIDER_TIMEOUT_SECONDS: u64 = 5;

#[derive(Clone, Debug)]
pub struct QrisOtomatisConfig {
    pub base_url: Url,
    pub merchant_uuid: String,
    pub client_name: String,
    pub client_key: String,
    pub timeout: Duration,
}

impl PartialEq for QrisOtomatisConfig {
    fn eq(&self, other: &Self) -> bool {
        self.base_url == other.base_url
            && self.merchant_uuid == other.merchant_uuid
            && self.client_name == other.client_name
            && self.client_key == other.client_key
            && self.timeout == other.timeout
    }
}

impl Eq for QrisOtomatisConfig {}

impl QrisOtomatisConfig {
    pub fn from_app_config(config: &Config) -> Result<Self, AppError> {
        let base_url = config.external_api_url.trim();
        let merchant_uuid = config.external_api_uuid.trim();
        let client_name = config.external_api_client.trim();
        let client_key = config.external_api_secret.trim();

        if base_url.is_empty() {
            return Err(AppError::Config(
                "EXTERNAL_API_URL must not be empty".into(),
            ));
        }
        if merchant_uuid.is_empty() {
            return Err(AppError::Config(
                "EXTERNAL_API_UUID must not be empty".into(),
            ));
        }
        if client_name.is_empty() {
            return Err(AppError::Config(
                "EXTERNAL_API_CLIENT must not be empty".into(),
            ));
        }
        if client_key.is_empty() {
            return Err(AppError::Config(
                "EXTERNAL_API_SECRET must not be empty".into(),
            ));
        }
        if config.external_api_timeout_seconds == 0 {
            return Err(AppError::Config(
                "EXTERNAL_API_TIMEOUT_SECONDS must be greater than zero".into(),
            ));
        }

        let mut parsed_base_url = Url::parse(base_url)
            .map_err(|error| AppError::Config(format!("EXTERNAL_API_URL is invalid: {error}")))?;
        if !parsed_base_url.path().ends_with('/') {
            let next_path = format!("{}/", parsed_base_url.path().trim_end_matches('/'));
            parsed_base_url.set_path(&next_path);
        }

        Ok(Self {
            base_url: parsed_base_url,
            merchant_uuid: merchant_uuid.to_string(),
            client_name: client_name.to_string(),
            client_key: client_key.to_string(),
            timeout: Duration::from_secs(config.external_api_timeout_seconds),
        })
    }
}
