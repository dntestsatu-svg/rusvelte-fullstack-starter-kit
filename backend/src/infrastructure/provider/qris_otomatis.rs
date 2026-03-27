use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::bootstrap::config::Config;
use crate::modules::payments::application::provider::{
    GenerateQrisRequest, GeneratedQris, QrisProviderGateway,
};
use crate::shared::error::AppError;

pub const DEFAULT_PROVIDER_TIMEOUT_SECONDS: u64 = 5;
pub const DEFAULT_PROVIDER_MAX_RETRIES: usize = 1;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QrisOtomatisConfig {
    pub base_url: String,
    pub merchant_uuid: String,
    pub client_name: String,
    pub client_key: String,
    pub timeout: Duration,
    pub max_retries: usize,
}

impl QrisOtomatisConfig {
    pub fn from_app_config(config: &Config) -> Self {
        Self {
            base_url: config.external_api_url.clone(),
            merchant_uuid: config.external_api_uuid.clone(),
            client_name: config.external_api_client.clone(),
            client_key: config.external_api_secret.clone(),
            timeout: Duration::from_secs(DEFAULT_PROVIDER_TIMEOUT_SECONDS),
            max_retries: DEFAULT_PROVIDER_MAX_RETRIES,
        }
    }
}

pub struct QrisOtomatisProvider {
    client: reqwest::Client,
    config: QrisOtomatisConfig,
}

impl QrisOtomatisProvider {
    pub fn new(config: QrisOtomatisConfig) -> Result<Self, AppError> {
        let client = reqwest::Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|error| {
                AppError::Config(format!("Invalid provider client config: {error}"))
            })?;

        Ok(Self { client, config })
    }

    async fn send_generate_request(
        &self,
        request: &GenerateQrisProviderRequest<'_>,
    ) -> Result<reqwest::Response, reqwest::Error> {
        let url = format!(
            "{}/api/generate",
            self.config.base_url.trim_end_matches('/')
        );

        self.client
            .post(url)
            .header("accept", "application/json")
            .header("content-type", "application/json")
            .json(request)
            .send()
            .await
    }
}

#[async_trait]
impl QrisProviderGateway for QrisOtomatisProvider {
    async fn generate_qris(&self, request: GenerateQrisRequest) -> Result<GeneratedQris, AppError> {
        let payload = GenerateQrisProviderRequest {
            username: request.username.as_str(),
            amount: request.amount,
            uuid: self.config.merchant_uuid.as_str(),
            expire: request.expire_seconds,
            custom_ref: request.custom_ref.as_deref(),
        };

        let mut attempt = 0usize;
        loop {
            match self.send_generate_request(&payload).await {
                Ok(http_response) => {
                    if http_response.status().is_server_error() && attempt < self.config.max_retries
                    {
                        attempt += 1;
                        continue;
                    }

                    let status = http_response.status();
                    let provider_response: GenerateQrisProviderResponse =
                        http_response.json().await.map_err(|_| {
                            AppError::Internal(anyhow::anyhow!(
                                "Provider generate returned an invalid response"
                            ))
                        })?;

                    if provider_response.status {
                        let provider_trx_id = provider_response.trx_id.ok_or_else(|| {
                            AppError::Internal(anyhow::anyhow!(
                                "Provider generate succeeded without trx_id"
                            ))
                        })?;
                        let qris_payload = provider_response.data.ok_or_else(|| {
                            AppError::Internal(anyhow::anyhow!(
                                "Provider generate succeeded without QRIS payload"
                            ))
                        })?;

                        return Ok(GeneratedQris {
                            provider_trx_id,
                            qris_payload,
                        });
                    }

                    let provider_message = provider_response
                        .error
                        .unwrap_or_else(|| "Unknown provider error".into());
                    if status.is_server_error() {
                        return Err(AppError::Internal(anyhow::anyhow!(
                            "Provider generate request failed"
                        )));
                    }

                    return Err(AppError::BadRequest(format!(
                        "Provider rejected QRIS generate request: {provider_message}"
                    )));
                }
                Err(error)
                    if attempt < self.config.max_retries
                        && (error.is_timeout() || error.is_connect() || error.is_request()) =>
                {
                    attempt += 1;
                }
                Err(_) => {
                    return Err(AppError::Internal(anyhow::anyhow!(
                        "Provider generate request failed"
                    )));
                }
            }
        }
    }
}

#[derive(Debug, Serialize)]
struct GenerateQrisProviderRequest<'a> {
    username: &'a str,
    amount: i64,
    uuid: &'a str,
    expire: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    custom_ref: Option<&'a str>,
}

#[derive(Debug, Deserialize)]
struct GenerateQrisProviderResponse {
    status: bool,
    data: Option<String>,
    trx_id: Option<String>,
    error: Option<String>,
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    use axum::{extract::State, routing::post, Json, Router};
    use reqwest::StatusCode;
    use serde_json::{json, Value};
    use tokio::net::TcpListener;

    use super::*;

    #[derive(Clone)]
    struct TestProviderState {
        requests: Arc<std::sync::Mutex<Vec<Value>>>,
        call_count: Arc<AtomicUsize>,
        first_response_status: StatusCode,
    }

    async fn spawn_provider_server(state: TestProviderState) -> SocketAddr {
        async fn handle_generate(
            State(state): State<TestProviderState>,
            Json(payload): Json<Value>,
        ) -> (StatusCode, Json<Value>) {
            state.call_count.fetch_add(1, Ordering::SeqCst);
            state.requests.lock().unwrap().push(payload);

            if state.call_count.load(Ordering::SeqCst) == 1
                && state.first_response_status == StatusCode::INTERNAL_SERVER_ERROR
            {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "status": false,
                        "error": "temporary upstream issue"
                    })),
                );
            }

            (
                StatusCode::OK,
                Json(json!({
                    "status": true,
                    "data": "qris-payload",
                    "trx_id": "trx-123"
                })),
            )
        }

        let app = Router::new()
            .route("/api/generate", post(handle_generate))
            .with_state(state);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        addr
    }

    #[tokio::test]
    async fn generate_qris_maps_success_payload_and_uses_config_fields() {
        let requests = Arc::new(std::sync::Mutex::new(Vec::new()));
        let call_count = Arc::new(AtomicUsize::new(0));
        let addr = spawn_provider_server(TestProviderState {
            requests: requests.clone(),
            call_count,
            first_response_status: StatusCode::OK,
        })
        .await;
        let provider = QrisOtomatisProvider::new(QrisOtomatisConfig {
            base_url: format!("http://{addr}"),
            merchant_uuid: "merchant-uuid".into(),
            client_name: "client-name".into(),
            client_key: "client-key".into(),
            timeout: Duration::from_secs(5),
            max_retries: 1,
        })
        .unwrap();

        let generated = provider
            .generate_qris(GenerateQrisRequest {
                username: "store-user".into(),
                amount: 15_000,
                expire_seconds: 300,
                custom_ref: Some("ref-1".into()),
            })
            .await
            .unwrap();

        assert_eq!(generated.provider_trx_id, "trx-123");
        assert_eq!(generated.qris_payload, "qris-payload");

        let payloads = requests.lock().unwrap();
        assert_eq!(payloads.len(), 1);
        assert_eq!(payloads[0]["username"], json!("store-user"));
        assert_eq!(payloads[0]["amount"], json!(15_000));
        assert_eq!(payloads[0]["uuid"], json!("merchant-uuid"));
        assert_eq!(payloads[0]["expire"], json!(300));
        assert_eq!(payloads[0]["custom_ref"], json!("ref-1"));
    }

    #[tokio::test]
    async fn generate_qris_maps_provider_failure_to_app_error() {
        async fn failing_generate(Json(_payload): Json<Value>) -> Json<Value> {
            Json(json!({
                "status": false,
                "error": "maintenance"
            }))
        }

        let app = Router::new().route("/api/generate", post(failing_generate));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let provider = QrisOtomatisProvider::new(QrisOtomatisConfig {
            base_url: format!("http://{addr}"),
            merchant_uuid: "merchant-uuid".into(),
            client_name: "client-name".into(),
            client_key: "client-key".into(),
            timeout: Duration::from_secs(5),
            max_retries: 1,
        })
        .unwrap();

        let error = provider
            .generate_qris(GenerateQrisRequest {
                username: "store-user".into(),
                amount: 15_000,
                expire_seconds: 300,
                custom_ref: None,
            })
            .await
            .unwrap_err();

        assert!(matches!(error, AppError::BadRequest(_)));
    }

    #[tokio::test]
    async fn generate_qris_retries_once_on_server_error() {
        let requests = Arc::new(std::sync::Mutex::new(Vec::new()));
        let call_count = Arc::new(AtomicUsize::new(0));
        let addr = spawn_provider_server(TestProviderState {
            requests,
            call_count: call_count.clone(),
            first_response_status: StatusCode::INTERNAL_SERVER_ERROR,
        })
        .await;
        let provider = QrisOtomatisProvider::new(QrisOtomatisConfig {
            base_url: format!("http://{addr}"),
            merchant_uuid: "merchant-uuid".into(),
            client_name: "client-name".into(),
            client_key: "client-key".into(),
            timeout: Duration::from_secs(5),
            max_retries: 1,
        })
        .unwrap();

        let generated = provider
            .generate_qris(GenerateQrisRequest {
                username: "store-user".into(),
                amount: 15_000,
                expire_seconds: 300,
                custom_ref: None,
            })
            .await
            .unwrap();

        assert_eq!(generated.provider_trx_id, "trx-123");
        assert_eq!(call_count.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn provider_config_uses_default_timeout_and_retry() {
        let config = Config {
            port: 8080,
            database_url: "postgres://localhost".into(),
            redis_url: "redis://127.0.0.1".into(),
            log_level: "info".into(),
            external_api_url: "https://provider.example".into(),
            external_api_uuid: "uuid-123".into(),
            external_api_client: "client-123".into(),
            external_api_secret: "secret-123".into(),
        };

        let provider_config = QrisOtomatisConfig::from_app_config(&config);
        assert_eq!(provider_config.timeout, Duration::from_secs(5));
        assert_eq!(provider_config.max_retries, 1);
        assert_eq!(provider_config.base_url, "https://provider.example");
        assert_eq!(provider_config.merchant_uuid, "uuid-123");
    }
}
