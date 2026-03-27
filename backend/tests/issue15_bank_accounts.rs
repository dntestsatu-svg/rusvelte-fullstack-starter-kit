use std::sync::{Arc, Mutex};

use anyhow::anyhow;
use async_trait::async_trait;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    middleware, Router,
};
use backend::bootstrap::config::Config;
use backend::bootstrap::state::AppState;
use backend::infrastructure::redis::RedisPool;
use backend::infrastructure::security::captcha::NoOpCaptchaVerifier;
use backend::modules::auth::application::dto::{SessionContext, UserProfile};
use backend::modules::auth::application::service::AuthService;
use backend::modules::auth::domain::repository::AuthRepository;
use backend::modules::auth::domain::session::Session;
use backend::modules::auth::domain::user::AuthUser;
use backend::modules::auth::interfaces::http::middlewares::csrf_middleware;
use backend::modules::balances::application::service::StoreBalanceService;
use backend::modules::balances::infrastructure::repository::SqlxStoreBalanceRepository;
use backend::modules::notifications::application::service::NotificationService;
use backend::modules::notifications::infrastructure::repository::SqlxNotificationRepository;
use backend::modules::payments::application::idempotency::PaymentIdempotencyService;
use backend::modules::payments::application::provider::{
    CheckDisbursementStatusRequest, CheckPaymentStatusRequest, CheckedDisbursementStatus,
    CheckedPaymentStatus, GenerateQrisRequest, GeneratedQris, GetBalanceRequest,
    InquiryBankRequest, InquiryBankResult, PaymentProviderGateway, ProviderBalanceSnapshot,
    TransferRequest, TransferResult,
};
use backend::modules::payments::application::service::PaymentService;
use backend::modules::payments::infrastructure::repository::SqlxPaymentRepository;
use backend::modules::realtime::application::service::RealtimeService;
use backend::modules::settlements::application::service::SettlementService;
use backend::modules::settlements::infrastructure::repository::SqlxSettlementRepository;
use backend::modules::store_banks::application::service::StoreBankService;
use backend::modules::store_banks::infrastructure::repository::SqlxStoreBankRepository;
use backend::modules::store_tokens::application::service::StoreTokenService;
use backend::modules::store_tokens::domain::entity::{NewStoreApiTokenRecord, StoreApiTokenRecord};
use backend::modules::store_tokens::domain::repository::StoreTokenRepository;
use backend::modules::stores::application::service::StoreService;
use backend::modules::stores::infrastructure::repository::SqlxStoreRepository;
use backend::modules::support::application::service::SupportService;
use backend::modules::support::infrastructure::repository::SupportRepository;
use backend::modules::users::application::service::UserService;
use backend::modules::users::infrastructure::repository::SqlxUserRepository;
use backend::shared::audit::{AuditLogEntry, AuditLogRepository};
use backend::shared::auth::StoreRole;
use backend::shared::error::AppError;
use bb8::Pool;
use bb8_redis::RedisConnectionManager;
use chrono::Utc;
use serde_json::{json, Value};
use sqlx::{postgres::PgPoolOptions, PgPool, Row};
use tower::ServiceExt;
use uuid::Uuid;

#[derive(Default)]
struct NoopAuditRepository;

#[async_trait]
impl AuditLogRepository for NoopAuditRepository {
    async fn insert(&self, _entry: AuditLogEntry) -> anyhow::Result<()> {
        Ok(())
    }
}

struct MockAuthRepository;

#[async_trait]
impl AuthRepository for MockAuthRepository {
    async fn find_user_by_email(&self, _email: &str) -> anyhow::Result<Option<AuthUser>> {
        Ok(None)
    }

    async fn find_user_by_id(&self, _id: Uuid) -> anyhow::Result<Option<AuthUser>> {
        Ok(None)
    }

    async fn update_last_login(&self, _id: Uuid) -> anyhow::Result<()> {
        Ok(())
    }

    async fn create_session(&self, _session: &Session) -> anyhow::Result<()> {
        Ok(())
    }

    async fn find_session_by_id(&self, _id: Uuid) -> anyhow::Result<Option<Session>> {
        Ok(None)
    }

    async fn delete_session(&self, _id: Uuid) -> anyhow::Result<()> {
        Ok(())
    }

    async fn delete_expired_sessions(&self) -> anyhow::Result<()> {
        Ok(())
    }
}

#[derive(Default)]
struct NoopStoreTokenRepository;

#[async_trait]
impl StoreTokenRepository for NoopStoreTokenRepository {
    async fn store_exists(&self, _store_id: Uuid) -> anyhow::Result<bool> {
        Ok(false)
    }

    async fn list_active_tokens(
        &self,
        _store_id: Uuid,
    ) -> anyhow::Result<Vec<StoreApiTokenRecord>> {
        Ok(vec![])
    }

    async fn insert_token(
        &self,
        _token: NewStoreApiTokenRecord,
    ) -> anyhow::Result<StoreApiTokenRecord> {
        Err(anyhow!("store tokens are not used in bank account tests"))
    }

    async fn find_token_by_lookup_prefix(
        &self,
        _token_prefix: &str,
    ) -> anyhow::Result<Option<StoreApiTokenRecord>> {
        Ok(None)
    }

    async fn find_token_by_id(
        &self,
        _store_id: Uuid,
        _token_id: Uuid,
    ) -> anyhow::Result<Option<StoreApiTokenRecord>> {
        Ok(None)
    }

    async fn mark_token_revoked(
        &self,
        _store_id: Uuid,
        _token_id: Uuid,
        _revoked_at: chrono::DateTime<Utc>,
    ) -> anyhow::Result<bool> {
        Ok(false)
    }

    async fn touch_last_used_at(
        &self,
        _token_id: Uuid,
        _last_used_at: chrono::DateTime<Utc>,
    ) -> anyhow::Result<()> {
        Ok(())
    }
}

#[derive(Clone)]
struct TestProvider {
    inquiry_result: Arc<Mutex<InquiryBehavior>>,
}

impl Default for TestProvider {
    fn default() -> Self {
        Self {
            inquiry_result: Arc::new(Mutex::new(InquiryBehavior::Success(InquiryBankResult {
                account_number: "1234567890".into(),
                account_name: "Alice Owner".into(),
                bank_code: "014".into(),
                bank_name: "PT. BANK CENTRAL ASIA TBK".into(),
                partner_ref_no: "partner-ref-15".into(),
                vendor_ref_no: Some("vendor-ref-15".into()),
                amount: 10_000,
                fee: 1800,
                inquiry_id: 1550,
            }))),
        }
    }
}

#[derive(Clone)]
enum InquiryBehavior {
    Success(InquiryBankResult),
    Error(String),
}

#[async_trait]
impl PaymentProviderGateway for TestProvider {
    async fn generate_qris(&self, _request: GenerateQrisRequest) -> Result<GeneratedQris, AppError> {
        Err(AppError::Internal(anyhow!("not used in bank account tests")))
    }

    async fn check_payment_status(
        &self,
        _request: CheckPaymentStatusRequest,
    ) -> Result<CheckedPaymentStatus, AppError> {
        Err(AppError::Internal(anyhow!("not used in bank account tests")))
    }

    async fn inquiry_bank(
        &self,
        request: InquiryBankRequest,
    ) -> Result<InquiryBankResult, AppError> {
        match self.inquiry_result.lock().unwrap().clone() {
            InquiryBehavior::Success(value) => Ok(InquiryBankResult {
                account_number: request.account_number,
                bank_code: request.bank_code,
                amount: request.amount,
                ..value
            }),
            InquiryBehavior::Error(message) => Err(AppError::BadRequest(message)),
        }
    }

    async fn transfer(&self, _request: TransferRequest) -> Result<TransferResult, AppError> {
        Err(AppError::Internal(anyhow!("not used in bank account tests")))
    }

    async fn check_disbursement_status(
        &self,
        _request: CheckDisbursementStatusRequest,
    ) -> Result<CheckedDisbursementStatus, AppError> {
        Err(AppError::Internal(anyhow!("not used in bank account tests")))
    }

    async fn get_balance(&self, _request: GetBalanceRequest) -> Result<ProviderBalanceSnapshot, AppError> {
        Err(AppError::Internal(anyhow!("not used in bank account tests")))
    }
}

#[derive(Clone)]
struct TestHarness {
    state: AppState,
    db: PgPool,
    provider: TestProvider,
}

#[derive(Debug, Clone)]
struct SeededUser {
    id: Uuid,
}

#[derive(Debug, Clone)]
struct SeededStore {
    id: Uuid,
}

fn build_redis_pool(config: &Config) -> RedisPool {
    let manager = RedisConnectionManager::new(config.redis_url.clone()).unwrap();
    Pool::builder().build_unchecked(manager)
}

async fn build_harness() -> TestHarness {
    let base_config = Config::from_env().unwrap();
    let db = PgPoolOptions::new()
        .max_connections(5)
        .connect(&base_config.database_url)
        .await
        .unwrap();
    let redis = build_redis_pool(&base_config);

    let audit_repository = Arc::new(NoopAuditRepository);
    let user_repository = Arc::new(SqlxUserRepository::new(db.clone()));
    let store_repository = Arc::new(SqlxStoreRepository::new(db.clone()));
    let balance_repository = Arc::new(SqlxStoreBalanceRepository::new(db.clone()));
    let payment_repository = Arc::new(SqlxPaymentRepository::new(db.clone()));
    let notification_repository = Arc::new(SqlxNotificationRepository::new(db.clone()));
    let settlement_repository = Arc::new(SqlxSettlementRepository::new(db.clone()));
    let store_bank_repository = Arc::new(SqlxStoreBankRepository::new(
        db.clone(),
        base_config.store_bank_account_encryption_key.clone(),
    ));
    let provider = TestProvider::default();

    let auth_service = Arc::new(AuthService::new(
        Arc::new(MockAuthRepository),
        Arc::new(NoOpCaptchaVerifier),
        redis.clone(),
    ));
    let user_service = Arc::new(UserService::new(
        user_repository.clone(),
        audit_repository.clone(),
    ));
    let store_service = Arc::new(StoreService::new(
        store_repository,
        user_repository,
        audit_repository.clone(),
    ));
    let balance_service = Arc::new(StoreBalanceService::new(balance_repository));
    let payment_service = Arc::new(PaymentService::new(
        payment_repository.clone(),
        Arc::new(provider.clone()),
    ));
    let payment_idempotency_service =
        Arc::new(PaymentIdempotencyService::new(payment_repository));
    let notification_service = Arc::new(NotificationService::new(notification_repository));
    let realtime_service = Arc::new(RealtimeService::new(64));
    let settlement_service = Arc::new(SettlementService::new(settlement_repository));
    let store_bank_service = Arc::new(StoreBankService::new(
        store_bank_repository,
        Arc::new(
            backend::modules::store_banks::infrastructure::cache::RedisStoreBankInquiryCache::new(
                redis.clone(),
            ),
        ),
        Arc::new(provider.clone()),
        Arc::new(backend::infrastructure::audit::SqlxAuditLogRepository::new(
            db.clone(),
        )),
    ));
    let support_service = Arc::new(SupportService::new(
        SupportRepository::new(db.clone()),
        Arc::new(NoOpCaptchaVerifier),
    ));
    let store_token_service = Arc::new(StoreTokenService::new(
        Arc::new(NoopStoreTokenRepository),
        audit_repository,
    ));

    let state = AppState {
        config: base_config,
        db: db.clone(),
        redis,
        auth_service,
        balance_service,
        notification_service,
        payment_idempotency_service,
        payment_service,
        realtime_service,
        settlement_service,
        store_bank_service,
        store_service,
        store_token_service,
        support_service,
        user_service,
    };

    TestHarness {
        state,
        db,
        provider,
    }
}

async fn insert_user(db: &PgPool, role: &str, label: &str) -> SeededUser {
    let id = Uuid::new_v4();
    let now = Utc::now();
    sqlx::query(
        r#"
        INSERT INTO users (
            id,
            name,
            email,
            password_hash,
            role,
            status,
            created_by,
            last_login_at,
            created_at,
            updated_at,
            deleted_at
        )
        VALUES ($1, $2, $3, 'test-hash', $4, 'active', NULL, NULL, $5, $5, NULL)
        "#,
    )
    .bind(id)
    .bind(format!("{label} {role}"))
    .bind(format!("{label}-{role}-{}@issue15.test", id.simple()))
    .bind(role)
    .bind(now)
    .execute(db)
    .await
    .unwrap();

    SeededUser { id }
}

async fn insert_store(db: &PgPool, owner_user_id: Uuid, label: &str) -> SeededStore {
    let id = Uuid::new_v4();
    let now = Utc::now();

    sqlx::query(
        r#"
        INSERT INTO stores (
            id,
            owner_user_id,
            name,
            slug,
            status,
            callback_url,
            callback_secret,
            provider_username,
            created_at,
            updated_at,
            deleted_at
        )
        VALUES ($1, $2, $3, $4, 'active', NULL, NULL, $5, $6, $6, NULL)
        "#,
    )
    .bind(id)
    .bind(owner_user_id)
    .bind(format!("{label} Store"))
    .bind(format!("{label}-{}", id.simple()))
    .bind(format!("terminal-{}", &id.simple().to_string()[..12]))
    .bind(now)
    .execute(db)
    .await
    .unwrap();

    insert_membership(db, id, owner_user_id, StoreRole::Owner).await;
    SeededStore { id }
}

async fn insert_membership(db: &PgPool, store_id: Uuid, user_id: Uuid, store_role: StoreRole) {
    let now = Utc::now();
    sqlx::query(
        r#"
        INSERT INTO store_members (
            id,
            store_id,
            user_id,
            store_role,
            status,
            invited_by,
            created_at,
            updated_at
        )
        VALUES ($1, $2, $3, $4, 'active', NULL, $5, $5)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(store_id)
    .bind(user_id)
    .bind(store_role.to_string())
    .bind(now)
    .execute(db)
    .await
    .unwrap();
}

fn bank_routes(state: AppState) -> Router {
    backend::modules::stores::routes(state.clone())
        .layer(middleware::from_fn(csrf_middleware))
        .with_state(state)
}

fn session_context(user_id: Uuid, role: &str, csrf_token: &str) -> SessionContext {
    SessionContext {
        session_id: Uuid::new_v4(),
        user: UserProfile {
            id: user_id,
            name: format!("User {role}"),
            email: format!("{role}@issue15.test"),
            role: role.into(),
            status: "active".into(),
        },
        csrf_token: csrf_token.into(),
    }
}

fn json_request(
    method: &str,
    uri: String,
    body: Value,
    user_id: Uuid,
    role: &str,
    csrf_token: &str,
    include_csrf: bool,
) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json");
    if include_csrf {
        builder = builder.header("X-CSRF-Token", csrf_token);
    }
    let mut request = builder.body(Body::from(body.to_string())).unwrap();
    request
        .extensions_mut()
        .insert(session_context(user_id, role, csrf_token));
    request
}

fn get_request(uri: String, user_id: Uuid, role: &str, csrf_token: &str) -> Request<Body> {
    let mut request = Request::builder()
        .method("GET")
        .uri(uri)
        .body(Body::empty())
        .unwrap();
    request
        .extensions_mut()
        .insert(session_context(user_id, role, csrf_token));
    request
}

#[tokio::test]
async fn inquiry_create_and_list_only_return_safe_metadata_and_encrypt_at_rest() {
    let harness = build_harness().await;
    let owner = insert_user(&harness.db, "user", "owner").await;
    let store = insert_store(&harness.db, owner.id, "banks-a").await;
    let app = bank_routes(harness.state.clone());

    let inquiry_response = app
        .clone()
        .oneshot(json_request(
            "POST",
            format!("/{}/banks/inquiry", store.id),
            json!({
                "bank_code": "014",
                "account_number": "1234567890"
            }),
            owner.id,
            "user",
            "csrf-owner",
            true,
        ))
        .await
        .unwrap();
    assert_eq!(inquiry_response.status(), StatusCode::OK);
    let inquiry_body = to_bytes(inquiry_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let inquiry_payload: Value = serde_json::from_slice(&inquiry_body).unwrap();
    assert_eq!(inquiry_payload["inquiry"]["account_holder_name"], json!("Alice Owner"));
    assert_eq!(inquiry_payload["inquiry"]["account_number_last4"], json!("7890"));
    assert!(inquiry_payload["inquiry"].get("account_number").is_none());

    *harness.provider.inquiry_result.lock().unwrap() = InquiryBehavior::Error(
        "Provider inquiry_bank rejected the request: should_not_be_called_again".into(),
    );

    let create_response = app
        .clone()
        .oneshot(json_request(
            "POST",
            format!("/{}/banks", store.id),
            json!({
                "bank_code": "014",
                "account_number": "1234567890",
                "is_default": true
            }),
            owner.id,
            "user",
            "csrf-owner",
            true,
        ))
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let create_body = to_bytes(create_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let create_payload: Value = serde_json::from_slice(&create_body).unwrap();
    let bank_id = Uuid::parse_str(create_payload["bank"]["id"].as_str().unwrap()).unwrap();
    assert_eq!(create_payload["bank"]["account_number_last4"], json!("7890"));
    assert!(create_payload["bank"].get("account_number_encrypted").is_none());

    let stored_row = sqlx::query(
        r#"
        SELECT account_number_encrypted, account_number_last4, is_default
        FROM store_bank_accounts
        WHERE id = $1
        "#,
    )
    .bind(bank_id)
    .fetch_one(&harness.db)
    .await
    .unwrap();
    let encrypted = stored_row.get::<String, _>("account_number_encrypted");
    assert_ne!(encrypted, "1234567890");
    assert_eq!(stored_row.get::<String, _>("account_number_last4"), "7890");
    assert!(stored_row.get::<bool, _>("is_default"));

    let list_response = app
        .clone()
        .oneshot(get_request(
            format!("/{}/banks", store.id),
            owner.id,
            "user",
            "csrf-owner",
        ))
        .await
        .unwrap();
    assert_eq!(list_response.status(), StatusCode::OK);
    let list_body = to_bytes(list_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let list_payload: Value = serde_json::from_slice(&list_body).unwrap();
    assert_eq!(list_payload["banks"][0]["bank_name"], json!("PT. BANK CENTRAL ASIA TBK"));
    assert_eq!(list_payload["banks"][0]["account_number_last4"], json!("7890"));

    let audit_payload = sqlx::query_scalar::<_, serde_json::Value>(
        r#"
        SELECT payload_json
        FROM audit_logs
        WHERE action = 'store.bank.create'
          AND payload_json ->> 'store_id' = $1
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(store.id.to_string())
    .fetch_one(&harness.db)
    .await
    .unwrap();
    let audit_payload_text = audit_payload.to_string();
    assert!(!audit_payload_text.contains("1234567890"));
    assert!(audit_payload_text.contains("7890"));
}

#[tokio::test]
async fn default_uniqueness_and_permissions_are_enforced() {
    let harness = build_harness().await;
    let owner = insert_user(&harness.db, "user", "owner").await;
    let other_owner = insert_user(&harness.db, "user", "other-owner").await;
    let manager = insert_user(&harness.db, "user", "manager").await;
    let superadmin = insert_user(&harness.db, "superadmin", "super").await;
    let dev = insert_user(&harness.db, "dev", "dev").await;
    let store = insert_store(&harness.db, owner.id, "banks-b").await;
    let other_store = insert_store(&harness.db, other_owner.id, "banks-c").await;
    insert_membership(&harness.db, store.id, manager.id, StoreRole::Manager).await;
    let app = bank_routes(harness.state.clone());

    for account_number in ["1234567890", "555566667777"] {
        let inquiry_response = app
            .clone()
            .oneshot(json_request(
                "POST",
                format!("/{}/banks/inquiry", store.id),
                json!({
                    "bank_code": "014",
                    "account_number": account_number
                }),
                owner.id,
                "user",
                "csrf-owner",
                true,
            ))
            .await
            .unwrap();
        assert_eq!(inquiry_response.status(), StatusCode::OK);

        let response = app
            .clone()
            .oneshot(json_request(
                "POST",
                format!("/{}/banks", store.id),
                json!({
                    "bank_code": "014",
                    "account_number": account_number,
                    "is_default": account_number == "1234567890"
                }),
                owner.id,
                "user",
                "csrf-owner",
                true,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
    }

    let second_bank_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        SELECT id
        FROM store_bank_accounts
        WHERE store_id = $1
          AND account_number_last4 = '7777'
        "#,
    )
    .bind(store.id)
    .fetch_one(&harness.db)
    .await
    .unwrap();

    let set_default_response = app
        .clone()
        .oneshot(json_request(
            "POST",
            format!("/{}/banks/{}/default", store.id, second_bank_id),
            json!({}),
            owner.id,
            "user",
            "csrf-owner",
            true,
        ))
        .await
        .unwrap();
    assert_eq!(set_default_response.status(), StatusCode::OK);

    let default_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM store_bank_accounts WHERE store_id = $1 AND is_default = TRUE",
    )
    .bind(store.id)
    .fetch_one(&harness.db)
    .await
    .unwrap();
    assert_eq!(default_count, 1);

    let superadmin_list = app
        .clone()
        .oneshot(get_request(
            format!("/{}/banks", store.id),
            superadmin.id,
            "superadmin",
            "csrf-super",
        ))
        .await
        .unwrap();
    assert_eq!(superadmin_list.status(), StatusCode::OK);

    let superadmin_create = app
        .clone()
        .oneshot(json_request(
            "POST",
            format!("/{}/banks", store.id),
            json!({
                "bank_code": "014",
                "account_number": "888899990000",
                "is_default": false
            }),
            superadmin.id,
            "superadmin",
            "csrf-super",
            true,
        ))
        .await
        .unwrap();
    assert_eq!(superadmin_create.status(), StatusCode::FORBIDDEN);

    let manager_list = app
        .clone()
        .oneshot(get_request(
            format!("/{}/banks", store.id),
            manager.id,
            "user",
            "csrf-manager",
        ))
        .await
        .unwrap();
    assert_eq!(manager_list.status(), StatusCode::FORBIDDEN);

    let cross_store = app
        .clone()
        .oneshot(get_request(
            format!("/{}/banks", other_store.id),
            owner.id,
            "user",
            "csrf-owner",
        ))
        .await
        .unwrap();
    assert_eq!(cross_store.status(), StatusCode::FORBIDDEN);

    let dev_create = app
        .clone()
        .oneshot(json_request(
            "POST",
            format!("/{}/banks/inquiry", other_store.id),
            json!({
                "bank_code": "014",
                "account_number": "999900001111"
            }),
            dev.id,
            "dev",
            "csrf-dev",
            true,
        ))
        .await
        .unwrap();
    assert_eq!(dev_create.status(), StatusCode::OK);

    let dev_create = app
        .clone()
        .oneshot(json_request(
            "POST",
            format!("/{}/banks", other_store.id),
            json!({
                "bank_code": "014",
                "account_number": "999900001111",
                "is_default": true
            }),
            dev.id,
            "dev",
            "csrf-dev",
            true,
        ))
        .await
        .unwrap();
    assert_eq!(dev_create.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn csrf_and_provider_failures_are_enforced_safely() {
    let harness = build_harness().await;
    let owner = insert_user(&harness.db, "user", "owner").await;
    let store = insert_store(&harness.db, owner.id, "banks-d").await;
    let app = bank_routes(harness.state.clone());

    let missing_csrf = app
        .clone()
        .oneshot(json_request(
            "POST",
            format!("/{}/banks/inquiry", store.id),
            json!({
                "bank_code": "014",
                "account_number": "1234567890"
            }),
            owner.id,
            "user",
            "csrf-owner",
            false,
        ))
        .await
        .unwrap();
    assert_eq!(missing_csrf.status(), StatusCode::FORBIDDEN);

    let create_without_inquiry = app
        .clone()
        .oneshot(json_request(
            "POST",
            format!("/{}/banks", store.id),
            json!({
                "bank_code": "014",
                "account_number": "1234567890",
                "is_default": true
            }),
            owner.id,
            "user",
            "csrf-owner",
            true,
        ))
        .await
        .unwrap();
    assert_eq!(create_without_inquiry.status(), StatusCode::BAD_REQUEST);

    *harness.provider.inquiry_result.lock().unwrap() = InquiryBehavior::Error(
        "Provider inquiry_bank rejected the request: Account not found".into(),
    );

    let provider_failure = app
        .clone()
        .oneshot(json_request(
            "POST",
            format!("/{}/banks/inquiry", store.id),
            json!({
                "bank_code": "014",
                "account_number": "1234567890"
            }),
            owner.id,
            "user",
            "csrf-owner",
            true,
        ))
        .await
        .unwrap();
    assert_eq!(provider_failure.status(), StatusCode::BAD_REQUEST);
}
