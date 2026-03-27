use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::shared::error::AppError;

use super::config::QrisOtomatisConfig;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetryPolicy {
    pub max_retries: usize,
    pub retry_on_timeout: bool,
    pub retry_on_connect: bool,
    pub retry_on_5xx: bool,
}

impl RetryPolicy {
    pub const fn no_retry() -> Self {
        Self {
            max_retries: 0,
            retry_on_timeout: false,
            retry_on_connect: false,
            retry_on_5xx: false,
        }
    }

    pub const fn read_only() -> Self {
        Self {
            max_retries: 1,
            retry_on_timeout: true,
            retry_on_connect: true,
            retry_on_5xx: true,
        }
    }

    pub const fn cautious_mutation() -> Self {
        Self {
            max_retries: 1,
            retry_on_timeout: false,
            retry_on_connect: false,
            retry_on_5xx: true,
        }
    }
}

#[derive(Debug)]
pub struct ProviderHttpResponse<T> {
    pub status: reqwest::StatusCode,
    pub body: T,
}

pub struct ProviderHttpClient {
    client: reqwest::Client,
    config: QrisOtomatisConfig,
}

impl ProviderHttpClient {
    pub fn new(config: QrisOtomatisConfig) -> Result<Self, AppError> {
        let client = reqwest::Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|error| {
                AppError::Config(format!("Invalid provider client config: {error}"))
            })?;

        Ok(Self { client, config })
    }

    pub fn config(&self) -> &QrisOtomatisConfig {
        &self.config
    }

    pub async fn post_json<TRequest, TResponse>(
        &self,
        path: &str,
        body: &TRequest,
        operation: &str,
        retry_policy: RetryPolicy,
    ) -> Result<ProviderHttpResponse<TResponse>, AppError>
    where
        TRequest: Serialize + ?Sized,
        TResponse: DeserializeOwned,
    {
        let url = self
            .config
            .base_url
            .join(path.trim_start_matches('/'))
            .map_err(|error| {
                AppError::Config(format!(
                    "Invalid provider endpoint for {operation}: {error}"
                ))
            })?;

        let mut attempt = 0usize;
        loop {
            match self
                .client
                .post(url.clone())
                .header("accept", "application/json")
                .header("content-type", "application/json")
                .json(body)
                .send()
                .await
            {
                Ok(response) => {
                    let status = response.status();
                    if status.is_server_error()
                        && retry_policy.retry_on_5xx
                        && attempt < retry_policy.max_retries
                    {
                        attempt += 1;
                        continue;
                    }

                    if status == reqwest::StatusCode::UNAUTHORIZED
                        || status == reqwest::StatusCode::FORBIDDEN
                    {
                        return Err(AppError::Config(format!(
                            "Provider authentication failed during {operation}"
                        )));
                    }

                    let body = response.json::<TResponse>().await.map_err(|_| {
                        AppError::Internal(anyhow::anyhow!(format!(
                            "Provider {operation} returned an invalid response"
                        )))
                    })?;

                    return Ok(ProviderHttpResponse { status, body });
                }
                Err(error)
                    if should_retry(&error, retry_policy) && attempt < retry_policy.max_retries =>
                {
                    attempt += 1;
                    continue;
                }
                Err(error) => {
                    if error.is_timeout() {
                        return Err(AppError::Internal(anyhow::anyhow!(format!(
                            "Provider {operation} timed out"
                        ))));
                    }
                    if error.is_connect() || error.is_request() {
                        return Err(AppError::Internal(anyhow::anyhow!(format!(
                            "Provider {operation} request failed"
                        ))));
                    }

                    return Err(AppError::Internal(anyhow::anyhow!(format!(
                        "Provider {operation} request failed"
                    ))));
                }
            }
        }
    }
}

fn should_retry(error: &reqwest::Error, retry_policy: RetryPolicy) -> bool {
    (retry_policy.retry_on_timeout && error.is_timeout())
        || (retry_policy.retry_on_connect && error.is_connect())
}
