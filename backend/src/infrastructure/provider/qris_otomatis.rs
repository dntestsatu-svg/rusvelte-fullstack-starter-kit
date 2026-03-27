use async_trait::async_trait;

use crate::modules::payments::application::provider::{
    CheckDisbursementStatusRequest, CheckPaymentStatusRequest, CheckedDisbursementStatus,
    CheckedPaymentStatus, GenerateQrisRequest, GeneratedQris, GetBalanceRequest,
    InquiryBankRequest, InquiryBankResult, PaymentProviderGateway, ProviderBalanceSnapshot,
    ProviderDisbursementStatus, ProviderPaymentStatus, TransferRequest, TransferResult,
};
use crate::shared::error::AppError;

use super::client::{ProviderHttpClient, RetryPolicy};
use super::config::QrisOtomatisConfig;
use super::raw::{
    CheckDisbursementStatusProviderRequest, CheckDisbursementStatusProviderResponse,
    CheckPaymentStatusProviderRequest, CheckPaymentStatusProviderResponse,
    GenerateQrisProviderRequest, GenerateQrisProviderResponse, GetBalanceProviderRequest,
    GetBalanceProviderResponse, InquiryBankProviderRequest, InquiryBankSuccessEnvelope,
    TransferProviderRequest, TransferProviderResponse,
};

pub struct QrisOtomatisProvider {
    http_client: ProviderHttpClient,
}

impl QrisOtomatisProvider {
    pub fn new(config: QrisOtomatisConfig) -> Result<Self, AppError> {
        Ok(Self {
            http_client: ProviderHttpClient::new(config)?,
        })
    }

    fn config(&self) -> &QrisOtomatisConfig {
        self.http_client.config()
    }
}

#[async_trait]
impl PaymentProviderGateway for QrisOtomatisProvider {
    async fn generate_qris(&self, request: GenerateQrisRequest) -> Result<GeneratedQris, AppError> {
        let payload = GenerateQrisProviderRequest {
            username: request.username.as_str(),
            amount: request.amount,
            uuid: self.config().merchant_uuid.as_str(),
            expire: request.expire_seconds,
            custom_ref: request.custom_ref.as_deref(),
        };

        let response = self
            .http_client
            .post_json::<_, GenerateQrisProviderResponse>(
                "api/generate",
                &payload,
                "generate_qris",
                RetryPolicy::cautious_mutation(),
            )
            .await?;

        if response.body.status {
            return Ok(GeneratedQris {
                provider_trx_id: response.body.trx_id.ok_or_else(|| {
                    AppError::Internal(anyhow::anyhow!(
                        "Provider generate_qris succeeded without trx_id"
                    ))
                })?,
                qris_payload: response.body.data.ok_or_else(|| {
                    AppError::Internal(anyhow::anyhow!(
                        "Provider generate_qris succeeded without qris payload"
                    ))
                })?,
            });
        }

        Err(provider_bad_request(
            "generate_qris",
            response.body.error.as_deref(),
        ))
    }

    async fn check_payment_status(
        &self,
        request: CheckPaymentStatusRequest,
    ) -> Result<CheckedPaymentStatus, AppError> {
        let payload = CheckPaymentStatusProviderRequest {
            uuid: self.config().merchant_uuid.as_str(),
            client: self.config().client_name.as_str(),
            client_key: self.config().client_key.as_str(),
        };

        let response = self
            .http_client
            .post_json::<_, CheckPaymentStatusProviderResponse>(
                &format!("api/checkstatus/v2/{}", request.provider_trx_id),
                &payload,
                "check_payment_status",
                RetryPolicy::read_only(),
            )
            .await?;

        match response.body {
            CheckPaymentStatusProviderResponse::Success(success) => Ok(CheckedPaymentStatus {
                amount: success.amount,
                provider_merchant_id: success.merchant_id,
                provider_trx_id: success.trx_id,
                provider_rrn: success.rrn,
                status: ProviderPaymentStatus::from_provider_value(&success.status),
                provider_created_at: success.created_at,
                provider_finished_at: success.finish_at,
            }),
            CheckPaymentStatusProviderResponse::Failure(failure) => {
                let _ = failure.status;
                Err(provider_bad_request(
                    "check_payment_status",
                    failure.error.as_deref(),
                ))
            }
        }
    }

    async fn inquiry_bank(
        &self,
        request: InquiryBankRequest,
    ) -> Result<InquiryBankResult, AppError> {
        let payload = InquiryBankProviderRequest {
            client: self.config().client_name.as_str(),
            client_key: self.config().client_key.as_str(),
            uuid: self.config().merchant_uuid.as_str(),
            amount: request.amount,
            bank_code: request.bank_code.as_str(),
            account_number: request.account_number.as_str(),
            transfer_type: request.disbursement_type.code(),
        };

        let response = self
            .http_client
            .post_json::<_, InquiryBankSuccessEnvelope>(
                "api/inquiry",
                &payload,
                "inquiry_bank",
                RetryPolicy::no_retry(),
            )
            .await?;

        if response.body.status {
            let data = response.body.data.ok_or_else(|| {
                AppError::Internal(anyhow::anyhow!(
                    "Provider inquiry_bank succeeded without data"
                ))
            })?;

            return Ok(InquiryBankResult {
                account_number: data.account_number,
                account_name: data.account_name,
                bank_code: data.bank_code,
                bank_name: data.bank_name,
                partner_ref_no: data.partner_ref_no,
                vendor_ref_no: data.vendor_ref_no.filter(|value| !value.trim().is_empty()),
                amount: data.amount,
                fee: data.fee,
                inquiry_id: data.inquiry_id,
            });
        }

        Err(provider_bad_request(
            "inquiry_bank",
            response.body.error.as_deref(),
        ))
    }

    async fn transfer(&self, request: TransferRequest) -> Result<TransferResult, AppError> {
        let payload = TransferProviderRequest {
            client: self.config().client_name.as_str(),
            client_key: self.config().client_key.as_str(),
            uuid: self.config().merchant_uuid.as_str(),
            amount: request.amount,
            bank_code: request.bank_code.as_str(),
            account_number: request.account_number.as_str(),
            transfer_type: request.disbursement_type.code(),
            inquiry_id: request.inquiry_id,
        };

        let response = self
            .http_client
            .post_json::<_, TransferProviderResponse>(
                "api/transfer",
                &payload,
                "transfer",
                RetryPolicy::no_retry(),
            )
            .await?;

        if response.body.status {
            return Ok(TransferResult { accepted: true });
        }

        Err(provider_bad_request(
            "transfer",
            response.body.error.as_deref(),
        ))
    }

    async fn check_disbursement_status(
        &self,
        request: CheckDisbursementStatusRequest,
    ) -> Result<CheckedDisbursementStatus, AppError> {
        let payload = CheckDisbursementStatusProviderRequest {
            client: self.config().client_name.as_str(),
            uuid: self.config().merchant_uuid.as_str(),
        };

        let response = self
            .http_client
            .post_json::<_, CheckDisbursementStatusProviderResponse>(
                &format!("api/disbursement/check-status/{}", request.partner_ref_no),
                &payload,
                "check_disbursement_status",
                RetryPolicy::read_only(),
            )
            .await?;

        match response.body {
            CheckDisbursementStatusProviderResponse::Success(success) => {
                Ok(CheckedDisbursementStatus {
                    amount: success.amount,
                    fee: success.fee,
                    partner_ref_no: success.partner_ref_no,
                    provider_merchant_id: success.merchant_uuid,
                    status: ProviderDisbursementStatus::from_provider_value(&success.status),
                })
            }
            CheckDisbursementStatusProviderResponse::Failure(failure) => {
                let _ = failure.status;
                Err(provider_bad_request(
                    "check_disbursement_status",
                    failure.error.as_deref(),
                ))
            }
        }
    }

    async fn get_balance(
        &self,
        _request: GetBalanceRequest,
    ) -> Result<ProviderBalanceSnapshot, AppError> {
        let payload = GetBalanceProviderRequest {
            client: self.config().client_name.as_str(),
        };

        let response = self
            .http_client
            .post_json::<_, GetBalanceProviderResponse>(
                &format!("api/balance/{}", self.config().merchant_uuid),
                &payload,
                "get_balance",
                RetryPolicy::read_only(),
            )
            .await?;

        match response.body {
            GetBalanceProviderResponse::Success(success) => {
                if !success.status.eq_ignore_ascii_case("success") {
                    return Err(AppError::Internal(anyhow::anyhow!(
                        "Provider get_balance returned an invalid success status"
                    )));
                }

                Ok(ProviderBalanceSnapshot {
                    provider_pending_balance: success.pending_balance,
                    provider_settle_balance: success.settle_balance,
                })
            }
            GetBalanceProviderResponse::Failure(failure) => {
                let _ = failure.status;
                Err(provider_bad_request(
                    "get_balance",
                    failure.error.as_deref(),
                ))
            }
        }
    }
}

fn provider_bad_request(operation: &str, message: Option<&str>) -> AppError {
    AppError::BadRequest(format!(
        "Provider {operation} failed: {}",
        message.unwrap_or("unknown provider error")
    ))
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::net::SocketAddr;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use axum::extract::{Path, State};
    use axum::routing::post;
    use axum::{Json, Router};
    use chrono::{TimeZone, Utc};
    use reqwest::StatusCode;
    use serde_json::{json, Value};
    use tokio::net::TcpListener;

    use super::*;
    use crate::bootstrap::config::Config;
    use crate::infrastructure::provider::config::DEFAULT_PROVIDER_TIMEOUT_SECONDS;
    use crate::modules::payments::application::provider::{
        ProviderDisbursementType, ProviderPaymentStatus,
    };

    #[derive(Clone, Debug)]
    struct MockResponse {
        status: StatusCode,
        body: Value,
        delay: Duration,
    }

    impl MockResponse {
        fn json(status: StatusCode, body: Value) -> Self {
            Self {
                status,
                body,
                delay: Duration::ZERO,
            }
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct RecordedRequest {
        path: String,
        body: Value,
    }

    #[derive(Clone, Default)]
    struct TestProviderState {
        recorded_requests: Arc<Mutex<Vec<RecordedRequest>>>,
        generate_responses: Arc<Mutex<VecDeque<MockResponse>>>,
        check_payment_status_responses: Arc<Mutex<VecDeque<MockResponse>>>,
        inquiry_responses: Arc<Mutex<VecDeque<MockResponse>>>,
        transfer_responses: Arc<Mutex<VecDeque<MockResponse>>>,
        check_disbursement_responses: Arc<Mutex<VecDeque<MockResponse>>>,
        balance_responses: Arc<Mutex<VecDeque<MockResponse>>>,
    }

    impl TestProviderState {
        fn push_generate(&self, response: MockResponse) {
            self.generate_responses.lock().unwrap().push_back(response);
        }

        fn push_check_payment_status(&self, response: MockResponse) {
            self.check_payment_status_responses
                .lock()
                .unwrap()
                .push_back(response);
        }

        fn push_inquiry(&self, response: MockResponse) {
            self.inquiry_responses.lock().unwrap().push_back(response);
        }

        fn push_transfer(&self, response: MockResponse) {
            self.transfer_responses.lock().unwrap().push_back(response);
        }

        fn push_check_disbursement(&self, response: MockResponse) {
            self.check_disbursement_responses
                .lock()
                .unwrap()
                .push_back(response);
        }

        fn push_balance(&self, response: MockResponse) {
            self.balance_responses.lock().unwrap().push_back(response);
        }

        fn recorded(&self) -> Vec<RecordedRequest> {
            self.recorded_requests.lock().unwrap().clone()
        }
    }

    async fn spawn_provider_server(state: TestProviderState) -> SocketAddr {
        async fn handle_generate(
            State(state): State<TestProviderState>,
            Json(payload): Json<Value>,
        ) -> (StatusCode, Json<Value>) {
            record_and_respond(
                &state.recorded_requests,
                "POST /api/generate".into(),
                payload,
                &state.generate_responses,
            )
            .await
        }

        async fn handle_check_payment_status(
            State(state): State<TestProviderState>,
            Path(trx_id): Path<String>,
            Json(payload): Json<Value>,
        ) -> (StatusCode, Json<Value>) {
            record_and_respond(
                &state.recorded_requests,
                format!("POST /api/checkstatus/v2/{trx_id}"),
                payload,
                &state.check_payment_status_responses,
            )
            .await
        }

        async fn handle_inquiry(
            State(state): State<TestProviderState>,
            Json(payload): Json<Value>,
        ) -> (StatusCode, Json<Value>) {
            record_and_respond(
                &state.recorded_requests,
                "POST /api/inquiry".into(),
                payload,
                &state.inquiry_responses,
            )
            .await
        }

        async fn handle_transfer(
            State(state): State<TestProviderState>,
            Json(payload): Json<Value>,
        ) -> (StatusCode, Json<Value>) {
            record_and_respond(
                &state.recorded_requests,
                "POST /api/transfer".into(),
                payload,
                &state.transfer_responses,
            )
            .await
        }

        async fn handle_check_disbursement_status(
            State(state): State<TestProviderState>,
            Path(partner_ref_no): Path<String>,
            Json(payload): Json<Value>,
        ) -> (StatusCode, Json<Value>) {
            record_and_respond(
                &state.recorded_requests,
                format!("POST /api/disbursement/check-status/{partner_ref_no}"),
                payload,
                &state.check_disbursement_responses,
            )
            .await
        }

        async fn handle_balance(
            State(state): State<TestProviderState>,
            Path(uuid): Path<String>,
            Json(payload): Json<Value>,
        ) -> (StatusCode, Json<Value>) {
            record_and_respond(
                &state.recorded_requests,
                format!("POST /api/balance/{uuid}"),
                payload,
                &state.balance_responses,
            )
            .await
        }

        let app = Router::new()
            .route("/api/generate", post(handle_generate))
            .route(
                "/api/checkstatus/v2/:trx_id",
                post(handle_check_payment_status),
            )
            .route("/api/inquiry", post(handle_inquiry))
            .route("/api/transfer", post(handle_transfer))
            .route(
                "/api/disbursement/check-status/:partner_ref_no",
                post(handle_check_disbursement_status),
            )
            .route("/api/balance/:uuid", post(handle_balance))
            .with_state(state);

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        addr
    }

    async fn record_and_respond(
        recorded_requests: &Arc<Mutex<Vec<RecordedRequest>>>,
        path: String,
        payload: Value,
        queue: &Arc<Mutex<VecDeque<MockResponse>>>,
    ) -> (StatusCode, Json<Value>) {
        recorded_requests.lock().unwrap().push(RecordedRequest {
            path,
            body: payload,
        });

        let response = queue.lock().unwrap().pop_front().unwrap_or_else(|| {
            MockResponse::json(
                StatusCode::NOT_FOUND,
                json!({
                    "status": false,
                    "error": "missing_mock"
                }),
            )
        });

        if !response.delay.is_zero() {
            tokio::time::sleep(response.delay).await;
        }

        (response.status, Json(response.body))
    }

    async fn build_provider(state: TestProviderState) -> (QrisOtomatisProvider, TestProviderState) {
        build_provider_with_timeout(state, Duration::from_secs(5)).await
    }

    async fn build_provider_with_timeout(
        state: TestProviderState,
        timeout: Duration,
    ) -> (QrisOtomatisProvider, TestProviderState) {
        let addr = spawn_provider_server(state.clone()).await;
        // Give the spawned mock server a brief moment to start accepting requests before
        // tests use very small client timeouts.
        tokio::time::sleep(Duration::from_millis(25)).await;
        let provider = QrisOtomatisProvider::new(QrisOtomatisConfig {
            base_url: reqwest::Url::parse(&format!("http://{addr}/")).unwrap(),
            merchant_uuid: "merchant-uuid".into(),
            client_name: "client-name".into(),
            client_key: "client-key".into(),
            timeout,
        })
        .unwrap();

        (provider, state)
    }

    async fn spawn_hanging_server() -> (SocketAddr, Arc<AtomicUsize>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let accepted_connections = Arc::new(AtomicUsize::new(0));
        let accepted_connections_for_task = accepted_connections.clone();

        tokio::spawn(async move {
            loop {
                let (stream, _) = listener.accept().await.unwrap();
                accepted_connections_for_task.fetch_add(1, Ordering::SeqCst);

                tokio::spawn(async move {
                    let _stream = stream;
                    tokio::time::sleep(Duration::from_millis(100)).await;
                });
            }
        });

        tokio::time::sleep(Duration::from_millis(25)).await;
        (addr, accepted_connections)
    }

    #[tokio::test]
    async fn generate_qris_success_mapping() {
        let state = TestProviderState::default();
        state.push_generate(MockResponse::json(
            StatusCode::OK,
            json!({
                "status": true,
                "data": "qris-payload",
                "trx_id": "trx-123"
            }),
        ));
        let (provider, state) = build_provider(state).await;

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

        let requests = state.recorded();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].path, "POST /api/generate");
        assert_eq!(requests[0].body["username"], json!("store-user"));
        assert_eq!(requests[0].body["amount"], json!(15_000));
        assert_eq!(requests[0].body["uuid"], json!("merchant-uuid"));
        assert_eq!(requests[0].body["expire"], json!(300));
        assert_eq!(requests[0].body["custom_ref"], json!("ref-1"));
    }

    #[tokio::test]
    async fn generate_qris_provider_logical_failure_is_mapped() {
        let state = TestProviderState::default();
        state.push_generate(MockResponse::json(
            StatusCode::OK,
            json!({
                "status": false,
                "error": "maintenance"
            }),
        ));
        let (provider, state) = build_provider(state).await;

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
        assert_eq!(state.recorded().len(), 1);
    }

    #[tokio::test]
    async fn generate_qris_malformed_response_is_rejected() {
        let state = TestProviderState::default();
        state.push_generate(MockResponse::json(
            StatusCode::OK,
            json!({
                "status": true,
                "trx_id": "trx-123"
            }),
        ));
        let (provider, _) = build_provider(state).await;

        let error = provider
            .generate_qris(GenerateQrisRequest {
                username: "store-user".into(),
                amount: 15_000,
                expire_seconds: 300,
                custom_ref: None,
            })
            .await
            .unwrap_err();

        assert!(matches!(error, AppError::Internal(_)));
    }

    #[tokio::test]
    async fn generate_qris_timeout_is_reported_without_retry() {
        let (addr, accepted_connections) = spawn_hanging_server().await;
        let provider = QrisOtomatisProvider::new(QrisOtomatisConfig {
            base_url: reqwest::Url::parse(&format!("http://{addr}/")).unwrap(),
            merchant_uuid: "merchant-uuid".into(),
            client_name: "client-name".into(),
            client_key: "client-key".into(),
            timeout: Duration::from_millis(10),
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

        println!(
            "{}",
            json!({
                "timeout_error": error.to_string(),
                "accepted_connections": accepted_connections.load(Ordering::SeqCst)
            })
        );
        assert!(matches!(error, AppError::Internal(_)));
        assert!(accepted_connections.load(Ordering::SeqCst) <= 1);
    }

    #[tokio::test]
    async fn generate_qris_retries_once_on_server_error_only() {
        let state = TestProviderState::default();
        state.push_generate(MockResponse::json(
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({
                "status": false,
                "error": "temporary upstream issue"
            }),
        ));
        state.push_generate(MockResponse::json(
            StatusCode::OK,
            json!({
                "status": true,
                "data": "qris-payload",
                "trx_id": "trx-123"
            }),
        ));
        let (provider, state) = build_provider(state).await;

        let result = provider
            .generate_qris(GenerateQrisRequest {
                username: "store-user".into(),
                amount: 15_000,
                expire_seconds: 300,
                custom_ref: None,
            })
            .await
            .unwrap();

        assert_eq!(result.provider_trx_id, "trx-123");
        assert_eq!(state.recorded().len(), 2);
    }

    #[tokio::test]
    async fn check_payment_status_success_mapping() {
        let state = TestProviderState::default();
        state.push_check_payment_status(MockResponse::json(
            StatusCode::OK,
            json!({
                "amount": 10000,
                "merchant_id": "merchant-uuid",
                "trx_id": "trx-123",
                "rrn": "rrn-123",
                "status": "success",
                "created_at": "2024-05-06T09:35:44.000Z",
                "finish_at": "2024-05-06T10:35:44.000Z"
            }),
        ));
        let (provider, state) = build_provider(state).await;

        let result = provider
            .check_payment_status(CheckPaymentStatusRequest {
                provider_trx_id: "trx-123".into(),
            })
            .await
            .unwrap();

        assert_eq!(result.amount, 10000);
        assert_eq!(result.provider_merchant_id, "merchant-uuid");
        assert_eq!(result.provider_rrn.as_deref(), Some("rrn-123"));
        assert_eq!(result.status, ProviderPaymentStatus::Success);
        assert_eq!(
            result.provider_created_at,
            Some(Utc.with_ymd_and_hms(2024, 5, 6, 9, 35, 44).unwrap())
        );

        let requests = state.recorded();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].path, "POST /api/checkstatus/v2/trx-123");
        assert_eq!(requests[0].body["uuid"], json!("merchant-uuid"));
        assert_eq!(requests[0].body["client"], json!("client-name"));
        assert_eq!(requests[0].body["client_key"], json!("client-key"));
    }

    #[tokio::test]
    async fn inquiry_bank_success_mapping() {
        let state = TestProviderState::default();
        state.push_inquiry(MockResponse::json(
            StatusCode::OK,
            json!({
                "status": true,
                "data": {
                    "account_number": "100009689749",
                    "account_name": "SISKA DAMAYANTI",
                    "bank_code": "542",
                    "bank_name": "PT. BANK ARTOS INDONESIA (Bank Jago)",
                    "partner_ref_no": "partner-1",
                    "vendor_ref_no": "",
                    "amount": 10000,
                    "fee": 1800,
                    "inquiry_id": 12345
                }
            }),
        ));
        let (provider, state) = build_provider(state).await;

        let result = provider
            .inquiry_bank(InquiryBankRequest {
                amount: 10000,
                bank_code: "542".into(),
                account_number: "100009689749".into(),
                disbursement_type: ProviderDisbursementType::Instant,
            })
            .await
            .unwrap();

        assert_eq!(result.account_name, "SISKA DAMAYANTI");
        assert_eq!(result.partner_ref_no, "partner-1");
        assert_eq!(result.fee, 1800);
        assert_eq!(result.vendor_ref_no, None);

        let requests = state.recorded();
        assert_eq!(requests[0].path, "POST /api/inquiry");
        assert_eq!(requests[0].body["client"], json!("client-name"));
        assert_eq!(requests[0].body["client_key"], json!("client-key"));
        assert_eq!(requests[0].body["uuid"], json!("merchant-uuid"));
        assert_eq!(requests[0].body["type"], json!(2));
    }

    #[tokio::test]
    async fn transfer_success_mapping() {
        let state = TestProviderState::default();
        state.push_transfer(MockResponse::json(
            StatusCode::OK,
            json!({
                "status": true
            }),
        ));
        let (provider, state) = build_provider(state).await;

        let result = provider
            .transfer(TransferRequest {
                amount: 25_000,
                bank_code: "014".into(),
                account_number: "0234567".into(),
                disbursement_type: ProviderDisbursementType::Instant,
                inquiry_id: 98765,
            })
            .await
            .unwrap();

        assert!(result.accepted);
        let requests = state.recorded();
        assert_eq!(requests[0].path, "POST /api/transfer");
        assert_eq!(requests[0].body["type"], json!(2));
        assert_eq!(requests[0].body["inquiry_id"], json!(98765));
    }

    #[tokio::test]
    async fn check_disbursement_status_success_mapping() {
        let state = TestProviderState::default();
        state.push_check_disbursement(MockResponse::json(
            StatusCode::OK,
            json!({
                "amount": 10000,
                "fee": 1800,
                "partner_ref_no": "partner-1",
                "merchant_uuid": "merchant-uuid",
                "status": "success"
            }),
        ));
        let (provider, state) = build_provider(state).await;

        let result = provider
            .check_disbursement_status(CheckDisbursementStatusRequest {
                partner_ref_no: "partner-1".into(),
            })
            .await
            .unwrap();

        assert_eq!(result.status, ProviderDisbursementStatus::Success);
        assert_eq!(result.fee, 1800);
        let requests = state.recorded();
        assert_eq!(
            requests[0].path,
            "POST /api/disbursement/check-status/partner-1"
        );
        assert_eq!(requests[0].body["client"], json!("client-name"));
        assert_eq!(requests[0].body["uuid"], json!("merchant-uuid"));
    }

    #[tokio::test]
    async fn get_balance_success_mapping() {
        let state = TestProviderState::default();
        state.push_balance(MockResponse::json(
            StatusCode::OK,
            json!({
                "status": "success",
                "pending_balance": 57726953,
                "settle_balance": 78407
            }),
        ));
        let (provider, state) = build_provider(state).await;

        let result = provider.get_balance(GetBalanceRequest).await.unwrap();

        assert_eq!(result.provider_pending_balance, 57_726_953);
        assert_eq!(result.provider_settle_balance, 78_407);
        let requests = state.recorded();
        assert_eq!(requests[0].path, "POST /api/balance/merchant-uuid");
        assert_eq!(requests[0].body["client"], json!("client-name"));
    }

    #[tokio::test]
    async fn provider_auth_failure_is_mapped_without_retry() {
        let state = TestProviderState::default();
        state.push_transfer(MockResponse::json(
            StatusCode::UNAUTHORIZED,
            json!({
                "status": false,
                "error": "invalid auth"
            }),
        ));
        let (provider, state) = build_provider(state).await;

        let error = provider
            .transfer(TransferRequest {
                amount: 25_000,
                bank_code: "014".into(),
                account_number: "0234567".into(),
                disbursement_type: ProviderDisbursementType::Instant,
                inquiry_id: 98765,
            })
            .await
            .unwrap_err();

        assert!(matches!(error, AppError::Config(_)));
        assert_eq!(state.recorded().len(), 1);
    }

    #[tokio::test]
    async fn read_only_operation_retries_once_on_transient_failure() {
        let state = TestProviderState::default();
        state.push_balance(MockResponse::json(
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({
                "status": false,
                "error": "temporary upstream issue"
            }),
        ));
        state.push_balance(MockResponse::json(
            StatusCode::OK,
            json!({
                "status": "success",
                "pending_balance": 100,
                "settle_balance": 200
            }),
        ));
        let (provider, state) = build_provider(state).await;

        let result = provider.get_balance(GetBalanceRequest).await.unwrap();

        println!(
            "{}",
            json!({
                "provider_pending_balance": result.provider_pending_balance,
                "retry_call_count": state.recorded().len(),
            })
        );
        assert_eq!(result.provider_pending_balance, 100);
        assert_eq!(state.recorded().len(), 2);
    }

    #[tokio::test]
    async fn mock_provider_round_trip_proof_for_all_operations() {
        let state = TestProviderState::default();
        state.push_generate(MockResponse::json(
            StatusCode::OK,
            json!({
                "status": true,
                "data": "qris-payload-proof",
                "trx_id": "trx-proof"
            }),
        ));
        state.push_check_payment_status(MockResponse::json(
            StatusCode::OK,
            json!({
                "amount": 15000,
                "merchant_id": "merchant-uuid",
                "trx_id": "trx-proof",
                "rrn": "rrn-proof",
                "status": "pending",
                "created_at": "2024-05-06T09:35:44.000Z",
                "finish_at": "2024-05-06T10:35:44.000Z"
            }),
        ));
        state.push_inquiry(MockResponse::json(
            StatusCode::OK,
            json!({
                "status": true,
                "data": {
                    "account_number": "100009689749",
                    "account_name": "SISKA DAMAYANTI",
                    "bank_code": "542",
                    "bank_name": "PT. BANK ARTOS INDONESIA (Bank Jago)",
                    "partner_ref_no": "partner-proof",
                    "vendor_ref_no": "vendor-proof",
                    "amount": 10000,
                    "fee": 1800,
                    "inquiry_id": 12345
                }
            }),
        ));
        state.push_transfer(MockResponse::json(
            StatusCode::OK,
            json!({
                "status": true
            }),
        ));
        state.push_check_disbursement(MockResponse::json(
            StatusCode::OK,
            json!({
                "amount": 10000,
                "fee": 1800,
                "partner_ref_no": "partner-proof",
                "merchant_uuid": "merchant-uuid",
                "status": "success"
            }),
        ));
        state.push_balance(MockResponse::json(
            StatusCode::OK,
            json!({
                "status": "success",
                "pending_balance": 123456,
                "settle_balance": 654321
            }),
        ));
        let (provider, state) = build_provider(state).await;

        let generated = provider
            .generate_qris(GenerateQrisRequest {
                username: "proof-store".into(),
                amount: 15_000,
                expire_seconds: 300,
                custom_ref: Some("proof-ref".into()),
            })
            .await
            .unwrap();
        let payment_status = provider
            .check_payment_status(CheckPaymentStatusRequest {
                provider_trx_id: "trx-proof".into(),
            })
            .await
            .unwrap();
        let inquiry = provider
            .inquiry_bank(InquiryBankRequest {
                amount: 10_000,
                bank_code: "542".into(),
                account_number: "100009689749".into(),
                disbursement_type: ProviderDisbursementType::Instant,
            })
            .await
            .unwrap();
        let transfer = provider
            .transfer(TransferRequest {
                amount: 25_000,
                bank_code: "014".into(),
                account_number: "0234567".into(),
                disbursement_type: ProviderDisbursementType::Instant,
                inquiry_id: 12345,
            })
            .await
            .unwrap();
        let disbursement_status = provider
            .check_disbursement_status(CheckDisbursementStatusRequest {
                partner_ref_no: "partner-proof".into(),
            })
            .await
            .unwrap();
        let balance = provider.get_balance(GetBalanceRequest).await.unwrap();

        let recorded_paths = state
            .recorded()
            .into_iter()
            .map(|request| request.path)
            .collect::<Vec<_>>();

        println!(
            "{}",
            json!({
                "generated": {
                    "provider_trx_id": generated.provider_trx_id,
                    "qris_payload": generated.qris_payload,
                },
                "payment_status": {
                    "status": format!("{:?}", payment_status.status),
                    "rrn": payment_status.provider_rrn,
                },
                "inquiry": {
                    "account_name": inquiry.account_name,
                    "partner_ref_no": inquiry.partner_ref_no,
                    "fee": inquiry.fee,
                },
                "transfer": {
                    "accepted": transfer.accepted,
                },
                "disbursement_status": {
                    "status": format!("{:?}", disbursement_status.status),
                    "fee": disbursement_status.fee,
                },
                "balance": {
                    "provider_pending_balance": balance.provider_pending_balance,
                    "provider_settle_balance": balance.provider_settle_balance,
                },
                "recorded_paths": recorded_paths,
            })
        );

        assert_eq!(payment_status.status, ProviderPaymentStatus::Pending);
        assert!(transfer.accepted);
        assert_eq!(
            disbursement_status.status,
            ProviderDisbursementStatus::Success
        );
        assert_eq!(balance.provider_pending_balance, 123_456);
    }

    #[test]
    fn provider_config_validates_required_values_and_default_timeout() {
        let config = Config {
            port: 8080,
            database_url: "postgres://localhost".into(),
            redis_url: "redis://127.0.0.1".into(),
            log_level: "info".into(),
            store_bank_account_encryption_key: "bank-test-key".into(),
            external_api_url: "https://provider.example".into(),
            external_api_uuid: "uuid-123".into(),
            external_api_client: "client-123".into(),
            external_api_secret: "secret-123".into(),
            external_api_timeout_seconds: DEFAULT_PROVIDER_TIMEOUT_SECONDS,
        };

        let provider_config = QrisOtomatisConfig::from_app_config(&config).unwrap();
        assert_eq!(provider_config.timeout, Duration::from_secs(5));
        assert_eq!(
            provider_config.base_url.as_str(),
            "https://provider.example/"
        );

        let invalid = Config {
            external_api_url: "not a url".into(),
            ..config
        };
        let error = QrisOtomatisConfig::from_app_config(&invalid).unwrap_err();
        assert!(matches!(error, AppError::Config(_)));
    }
}
