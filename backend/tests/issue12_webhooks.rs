use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::anyhow;
use async_trait::async_trait;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    middleware,
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
use backend::modules::payments::domain::entity::PAYMENT_PROVIDER_NAME;
use backend::modules::payments::infrastructure::repository::SqlxPaymentRepository;
use backend::modules::realtime::application::service::RealtimeService;
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
use backend::shared::money::payment_fee_breakdown;
use bb8::Pool;
use bb8_redis::RedisConnectionManager;
use chrono::{Duration as ChronoDuration, Utc};
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

    async fn list_active_tokens(&self, _store_id: Uuid) -> anyhow::Result<Vec<StoreApiTokenRecord>> {
        Ok(vec![])
    }

    async fn insert_token(&self, _token: NewStoreApiTokenRecord) -> anyhow::Result<StoreApiTokenRecord> {
        Err(anyhow!("store tokens are not used in issue12 integration tests"))
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

#[derive(Clone, Default)]
struct NoopProvider;

#[async_trait]
impl PaymentProviderGateway for NoopProvider {
    async fn generate_qris(&self, _request: GenerateQrisRequest) -> Result<GeneratedQris, AppError> {
        Err(AppError::Internal(anyhow!("not used in issue12 integration tests")))
    }

    async fn check_payment_status(
        &self,
        _request: CheckPaymentStatusRequest,
    ) -> Result<CheckedPaymentStatus, AppError> {
        Err(AppError::Internal(anyhow!("not used in issue12 integration tests")))
    }

    async fn inquiry_bank(&self, _request: InquiryBankRequest) -> Result<InquiryBankResult, AppError> {
        Err(AppError::Internal(anyhow!("not used in issue12 integration tests")))
    }

    async fn transfer(&self, _request: TransferRequest) -> Result<TransferResult, AppError> {
        Err(AppError::Internal(anyhow!("not used in issue12 integration tests")))
    }

    async fn check_disbursement_status(
        &self,
        _request: CheckDisbursementStatusRequest,
    ) -> Result<CheckedDisbursementStatus, AppError> {
        Err(AppError::Internal(anyhow!("not used in issue12 integration tests")))
    }

    async fn get_balance(&self, _request: GetBalanceRequest) -> Result<ProviderBalanceSnapshot, AppError> {
        Err(AppError::Internal(anyhow!("not used in issue12 integration tests")))
    }
}

#[derive(Clone)]
struct TestHarness {
    state: AppState,
    db: PgPool,
    merchant_id: String,
}

#[derive(Debug, Clone)]
struct SeededUser {
    id: Uuid,
    role: &'static str,
}

#[derive(Debug, Clone)]
struct SeededStore {
    id: Uuid,
    provider_terminal_id: String,
    callback_url: String,
}

#[derive(Debug, Clone)]
struct SeededPayment {
    id: Uuid,
    provider_trx_id: String,
    custom_ref: Option<String>,
    gross_amount: i64,
    platform_fee_amount: i64,
    store_pending_credit_amount: i64,
}

fn build_redis_pool() -> RedisPool {
    let base_config = Config::from_env().unwrap();
    let manager = RedisConnectionManager::new(base_config.redis_url).unwrap();
    Pool::builder().build_unchecked(manager)
}

async fn build_harness() -> TestHarness {
    let base_config = Config::from_env().unwrap();
    let db = PgPoolOptions::new()
        .max_connections(5)
        .connect(&base_config.database_url)
        .await
        .unwrap();
    let redis = build_redis_pool();
    let merchant_id = format!("merchant-{}", Uuid::new_v4().simple());
    let config = Config {
        port: 0,
        database_url: base_config.database_url,
        redis_url: base_config.redis_url,
        log_level: base_config.log_level,
        external_api_url: base_config.external_api_url,
        external_api_uuid: merchant_id.clone(),
        external_api_client: base_config.external_api_client,
        external_api_secret: base_config.external_api_secret,
        external_api_timeout_seconds: base_config.external_api_timeout_seconds,
    };

    let payment_repository = Arc::new(SqlxPaymentRepository::new(db.clone()));
    let user_repository = Arc::new(SqlxUserRepository::new(db.clone()));
    let notification_repository = Arc::new(SqlxNotificationRepository::new(db.clone()));
    let audit_repository = Arc::new(NoopAuditRepository);
    let captcha = Arc::new(NoOpCaptchaVerifier);

    let auth_service = Arc::new(AuthService::new(
        Arc::new(MockAuthRepository),
        captcha.clone(),
        redis.clone(),
    ));
    let user_service = Arc::new(UserService::new(
        user_repository.clone(),
        audit_repository.clone(),
    ));
    let store_service = Arc::new(StoreService::new(
        Arc::new(SqlxStoreRepository::new(db.clone())),
        user_repository,
        audit_repository.clone(),
    ));
    let support_service = Arc::new(SupportService::new(
        SupportRepository::new(db.clone()),
        captcha,
    ));
    let store_token_service = Arc::new(StoreTokenService::new(
        Arc::new(NoopStoreTokenRepository),
        audit_repository,
    ));
    let payment_service = Arc::new(PaymentService::new(
        payment_repository.clone(),
        Arc::new(NoopProvider),
    ));
    let payment_idempotency_service =
        Arc::new(PaymentIdempotencyService::new(payment_repository));
    let notification_service = Arc::new(NotificationService::new(notification_repository));
    let realtime_service = Arc::new(RealtimeService::new(256));

    let state = AppState {
        config,
        db: db.clone(),
        redis,
        auth_service,
        notification_service,
        payment_idempotency_service,
        payment_service,
        realtime_service,
        store_service,
        store_token_service,
        support_service,
        user_service,
    };

    TestHarness {
        state,
        db,
        merchant_id,
    }
}

async fn insert_user(db: &PgPool, role: &'static str) -> SeededUser {
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
    .bind(format!("User {role}"))
    .bind(format!("{role}-{}@issue12.test", id.simple()))
    .bind(role)
    .bind(now)
    .execute(db)
    .await
    .unwrap();

    SeededUser { id, role }
}

async fn insert_store(db: &PgPool, owner_user_id: Uuid, callback_enabled: bool) -> SeededStore {
    let id = Uuid::new_v4();
    let now = Utc::now();
    let provider_terminal_id = format!("terminal-{}", &id.simple().to_string()[..12]);
    let callback_url = format!("https://merchant.example/{}/callback", id.simple());
    let callback_secret = format!("secret-{}", id.simple());

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
        VALUES ($1, $2, $3, $4, 'active', $5, $6, $7, $8, $8, NULL)
        "#,
    )
    .bind(id)
    .bind(owner_user_id)
    .bind(format!("Store {}", &id.simple().to_string()[..8]))
    .bind(format!("store-{}", id.simple()))
    .bind(if callback_enabled {
        Some(callback_url.clone())
    } else {
        None
    })
    .bind(if callback_enabled {
        Some(callback_secret)
    } else {
        None
    })
    .bind(provider_terminal_id.clone())
    .bind(now)
    .execute(db)
    .await
    .unwrap();

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
        VALUES ($1, $2, $3, 'owner', 'active', NULL, $4, $4)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(id)
    .bind(owner_user_id)
    .bind(now)
    .execute(db)
    .await
    .unwrap();

    SeededStore {
        id,
        provider_terminal_id,
        callback_url,
    }
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
        ON CONFLICT (store_id, user_id)
        DO UPDATE SET
            store_role = EXCLUDED.store_role,
            status = 'active',
            updated_at = EXCLUDED.updated_at
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

async fn insert_pending_payment(
    db: &PgPool,
    store: &SeededStore,
    amount: i64,
    custom_ref: Option<&str>,
) -> SeededPayment {
    let id = Uuid::new_v4();
    let provider_trx_id = format!("trx-{}", id.simple());
    let breakdown = payment_fee_breakdown(amount);
    let now = Utc::now();
    let custom_ref = custom_ref.map(str::to_string);

    sqlx::query(
        r#"
        INSERT INTO payments (
            id,
            store_id,
            created_by_user_id,
            provider_name,
            provider_terminal_id,
            provider_trx_id,
            provider_rrn,
            merchant_order_id,
            custom_ref,
            gross_amount,
            platform_tx_fee_bps,
            platform_tx_fee_amount,
            store_pending_credit_amount,
            status,
            qris_payload,
            expired_at,
            provider_created_at,
            provider_finished_at,
            finalized_at,
            created_at,
            updated_at
        )
        VALUES (
            $1, $2, NULL, $3, $4, $5, NULL, $6, $7, $8, $9, $10, $11,
            'pending', $12, $13, $14, NULL, NULL, $14, $14
        )
        "#,
    )
    .bind(id)
    .bind(store.id)
    .bind(PAYMENT_PROVIDER_NAME)
    .bind(store.provider_terminal_id.clone())
    .bind(provider_trx_id.clone())
    .bind(format!("order-{}", id.simple()))
    .bind(custom_ref.clone())
    .bind(amount)
    .bind(breakdown.platform_fee_bps as i32)
    .bind(breakdown.platform_fee_amount)
    .bind(breakdown.store_pending_amount)
    .bind(format!("QRIS|{}", id.simple()))
    .bind(now + ChronoDuration::minutes(5))
    .bind(now)
    .execute(db)
    .await
    .unwrap();

    SeededPayment {
        id,
        provider_trx_id,
        custom_ref,
        gross_amount: amount,
        platform_fee_amount: breakdown.platform_fee_amount,
        store_pending_credit_amount: breakdown.store_pending_amount,
    }
}

fn webhook_request(payload: Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/provider")
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap()
}

fn payment_webhook_payload(
    merchant_id: &str,
    payment: &SeededPayment,
    terminal_id: &str,
    status: &str,
    custom_ref: Option<&str>,
) -> Value {
    json!({
        "amount": payment.gross_amount,
        "terminal_id": terminal_id,
        "merchant_id": merchant_id,
        "trx_id": payment.provider_trx_id,
        "rrn": format!("rrn-{}", payment.id.simple()),
        "custom_ref": custom_ref,
        "vendor": "provider-test",
        "status": status,
        "created_at": Utc::now().to_rfc3339(),
        "finish_at": Utc::now().to_rfc3339(),
    })
}

fn payout_webhook_payload(merchant_id: &str) -> Value {
    json!({
        "amount": 12500,
        "partner_ref_no": format!("partner-{}", Uuid::new_v4().simple()),
        "status": "success",
        "transaction_date": Utc::now().to_rfc3339(),
        "merchant_id": merchant_id,
    })
}

fn session_context(user: &SeededUser, csrf_token: &str) -> SessionContext {
    SessionContext {
        session_id: Uuid::new_v4(),
        user: UserProfile {
            id: user.id,
            name: format!("User {}", user.role),
            email: format!("{}@issue12.test", user.id.simple()),
            role: user.role.into(),
            status: "active".into(),
        },
        csrf_token: csrf_token.into(),
    }
}

#[tokio::test]
async fn smoke_issue12_harness_and_seed_rows() {
    let harness = build_harness().await;
    println!("built harness");
    let owner = insert_user(&harness.db, "user").await;
    println!("inserted user {}", owner.id);
    let store = insert_store(&harness.db, owner.id, true).await;
    println!("inserted store {}", store.id);
    let payment = insert_pending_payment(&harness.db, &store, 10_000, Some("smoke-ref")).await;
    println!("inserted payment {}", payment.id);

    let status = sqlx::query_scalar::<_, String>("SELECT status FROM payments WHERE id = $1")
        .bind(payment.id)
        .fetch_one(&harness.db)
        .await
        .unwrap();
    assert_eq!(status, "pending");
}

#[tokio::test]
async fn valid_payment_webhook_finalizes_once_and_writes_all_side_effects() {
    let harness = build_harness().await;
    let owner = insert_user(&harness.db, "user").await;
    let store = insert_store(&harness.db, owner.id, true).await;
    let payment = insert_pending_payment(&harness.db, &store, 10_001, Some("order-issue12")).await;
    let app = backend::modules::payments::webhook_routes(harness.state.clone())
        .with_state(harness.state.clone());
    let mut receiver = harness.state.realtime_service.subscribe();

    let started_at = Instant::now();
    let response = app
        .clone()
        .oneshot(webhook_request(payment_webhook_payload(
            &harness.merchant_id,
            &payment,
            &store.provider_terminal_id,
            "success",
            payment.custom_ref.as_deref(),
        )))
        .await
        .unwrap();
    let elapsed = started_at.elapsed();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(elapsed < Duration::from_secs(1));
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(serde_json::from_slice::<Value>(&body).unwrap(), json!({ "status": true }));

    let payment_event = tokio::time::timeout(Duration::from_secs(1), receiver.recv())
        .await
        .unwrap()
        .unwrap();
    let notification_event = tokio::time::timeout(Duration::from_secs(1), receiver.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(payment_event.event_type, "payment.updated");
    assert_eq!(notification_event.event_type, "notification.created");
    assert!(notification_event.target_user_ids.contains(&owner.id));

    let payment_row = sqlx::query(
        r#"
        SELECT status, provider_rrn, finalized_at, provider_terminal_id
        FROM payments
        WHERE id = $1
        "#,
    )
    .bind(payment.id)
    .fetch_one(&harness.db)
    .await
    .unwrap();
    assert_eq!(payment_row.get::<String, _>("status"), "success");
    assert!(payment_row.get::<Option<String>, _>("provider_rrn").is_some());
    assert!(payment_row
        .get::<Option<chrono::DateTime<Utc>>, _>("finalized_at")
        .is_some());
    assert_eq!(
        payment_row.get::<Option<String>, _>("provider_terminal_id").as_deref(),
        Some(store.provider_terminal_id.as_str())
    );

    let webhook_row = sqlx::query(
        r#"
        SELECT webhook_kind, is_verified, is_processed, processing_result
        FROM provider_webhook_events
        WHERE provider_trx_id = $1
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(&payment.provider_trx_id)
    .fetch_one(&harness.db)
    .await
    .unwrap();
    assert_eq!(webhook_row.get::<String, _>("webhook_kind"), "payment");
    assert!(webhook_row.get::<bool, _>("is_verified"));
    assert!(webhook_row.get::<bool, _>("is_processed"));
    assert_eq!(
        webhook_row
            .get::<Option<String>, _>("processing_result")
            .as_deref(),
        Some("payment_success")
    );

    let pending_balance =
        sqlx::query_scalar::<_, i64>("SELECT pending_balance FROM store_balance_summaries WHERE store_id = $1")
            .bind(store.id)
            .fetch_one(&harness.db)
            .await
            .unwrap();
    assert_eq!(pending_balance, payment.store_pending_credit_amount);

    let store_ledger_amount = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT amount
        FROM store_balance_ledger_entries
        WHERE store_id = $1
          AND related_id = $2
          AND entry_type = 'payment_success_credit_pending'
        "#,
    )
    .bind(store.id)
    .bind(payment.id)
    .fetch_one(&harness.db)
    .await
    .unwrap();
    assert_eq!(store_ledger_amount, payment.store_pending_credit_amount);

    let platform_ledger_amount = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT amount
        FROM platform_ledger_entries
        WHERE related_id = $1
          AND entry_type = 'payment_platform_fee_income'
        "#,
    )
    .bind(payment.id)
    .fetch_one(&harness.db)
    .await
    .unwrap();
    assert_eq!(platform_ledger_amount, payment.platform_fee_amount);

    let notification_count = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM user_notifications
        WHERE user_id = $1
          AND related_id = $2
        "#,
    )
    .bind(owner.id)
    .bind(payment.id)
    .fetch_one(&harness.db)
    .await
    .unwrap();
    assert_eq!(notification_count, 1);

    let callback_row = sqlx::query(
        r#"
        SELECT target_url, status, signature
        FROM callback_deliveries
        WHERE store_id = $1
          AND related_id = $2
        "#,
    )
    .bind(store.id)
    .bind(payment.id)
    .fetch_one(&harness.db)
    .await
    .unwrap();
    assert_eq!(callback_row.get::<String, _>("target_url"), store.callback_url);
    assert_eq!(callback_row.get::<String, _>("status"), "queued");
    assert!(!callback_row.get::<String, _>("signature").is_empty());
}

#[tokio::test]
async fn duplicate_webhook_is_recorded_but_does_not_double_process() {
    let harness = build_harness().await;
    let owner = insert_user(&harness.db, "user").await;
    let store = insert_store(&harness.db, owner.id, true).await;
    let payment = insert_pending_payment(&harness.db, &store, 12_345, Some("duplicate-ref")).await;
    let app = backend::modules::payments::webhook_routes(harness.state.clone())
        .with_state(harness.state.clone());
    let mut receiver = harness.state.realtime_service.subscribe();
    let payload = payment_webhook_payload(
        &harness.merchant_id,
        &payment,
        &store.provider_terminal_id,
        "success",
        payment.custom_ref.as_deref(),
    );

    let first = app.clone().oneshot(webhook_request(payload.clone())).await.unwrap();
    assert_eq!(first.status(), StatusCode::OK);
    let _ = tokio::time::timeout(Duration::from_secs(1), receiver.recv())
        .await
        .unwrap()
        .unwrap();
    let _ = tokio::time::timeout(Duration::from_secs(1), receiver.recv())
        .await
        .unwrap()
        .unwrap();

    let second = app.oneshot(webhook_request(payload)).await.unwrap();
    assert_eq!(second.status(), StatusCode::OK);
    let second_body = to_bytes(second.into_body(), usize::MAX).await.unwrap();
    assert_eq!(serde_json::from_slice::<Value>(&second_body).unwrap(), json!({ "status": true }));

    let no_extra_event = tokio::time::timeout(Duration::from_millis(150), receiver.recv()).await;
    assert!(no_extra_event.is_err());

    let webhook_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM provider_webhook_events WHERE provider_trx_id = $1",
    )
    .bind(&payment.provider_trx_id)
    .fetch_one(&harness.db)
    .await
    .unwrap();
    assert_eq!(webhook_count, 2);

    let last_processing_result = sqlx::query_scalar::<_, String>(
        r#"
        SELECT processing_result
        FROM provider_webhook_events
        WHERE provider_trx_id = $1
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(&payment.provider_trx_id)
    .fetch_one(&harness.db)
    .await
    .unwrap();
    assert_eq!(last_processing_result, "already_finalized");

    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM payment_events WHERE payment_id = $1",
        )
        .bind(payment.id)
        .fetch_one(&harness.db)
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM store_balance_ledger_entries WHERE related_id = $1",
        )
        .bind(payment.id)
        .fetch_one(&harness.db)
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM platform_ledger_entries WHERE related_id = $1",
        )
        .bind(payment.id)
        .fetch_one(&harness.db)
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM user_notifications WHERE user_id = $1 AND related_id = $2",
        )
        .bind(owner.id)
        .bind(payment.id)
        .fetch_one(&harness.db)
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM callback_deliveries WHERE store_id = $1 AND related_id = $2",
        )
        .bind(store.id)
        .bind(payment.id)
        .fetch_one(&harness.db)
        .await
        .unwrap(),
        1
    );
}

#[tokio::test]
async fn invalid_payment_correlations_are_rejected_safely_and_payout_shape_is_recorded() {
    let harness = build_harness().await;
    let owner = insert_user(&harness.db, "user").await;
    let store = insert_store(&harness.db, owner.id, false).await;
    let payment = insert_pending_payment(&harness.db, &store, 25_000, Some("expected-ref")).await;
    let app = backend::modules::payments::webhook_routes(harness.state.clone())
        .with_state(harness.state.clone());

    let invalid_payloads = vec![
        payment_webhook_payload(
            "wrong-merchant",
            &payment,
            &store.provider_terminal_id,
            "success",
            payment.custom_ref.as_deref(),
        ),
        payment_webhook_payload(
            &harness.merchant_id,
            &payment,
            "wrong-terminal",
            "success",
            payment.custom_ref.as_deref(),
        ),
        payment_webhook_payload(
            &harness.merchant_id,
            &payment,
            &store.provider_terminal_id,
            "success",
            Some("wrong-custom-ref"),
        ),
        json!({
            "amount": payment.gross_amount,
            "terminal_id": store.provider_terminal_id,
            "merchant_id": harness.merchant_id,
            "trx_id": format!("missing-{}", Uuid::new_v4().simple()),
            "status": "success",
        }),
    ];

    for payload in invalid_payloads {
        let response = app.clone().oneshot(webhook_request(payload)).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(serde_json::from_slice::<Value>(&body).unwrap(), json!({ "status": false }));
    }

    let payout_response = app
        .clone()
        .oneshot(webhook_request(payout_webhook_payload(&harness.merchant_id)))
        .await
        .unwrap();
    assert_eq!(payout_response.status(), StatusCode::OK);
    let payout_body = to_bytes(payout_response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(serde_json::from_slice::<Value>(&payout_body).unwrap(), json!({ "status": true }));

    let payment_status = sqlx::query_scalar::<_, String>("SELECT status FROM payments WHERE id = $1")
        .bind(payment.id)
        .fetch_one(&harness.db)
        .await
        .unwrap();
    assert_eq!(payment_status, "pending");

    let invalid_events = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM provider_webhook_events
        WHERE provider_trx_id = $1
          AND is_verified = FALSE
          AND is_processed = TRUE
        "#,
    )
    .bind(&payment.provider_trx_id)
    .fetch_one(&harness.db)
    .await
    .unwrap();
    assert_eq!(invalid_events, 3);

    let unknown_trx_invalid = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM provider_webhook_events WHERE processing_result = 'invalid_provider_trx_id'",
    )
    .fetch_one(&harness.db)
    .await
    .unwrap();
    assert!(unknown_trx_invalid >= 1);

    let payout_count = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM provider_webhook_events
        WHERE webhook_kind = 'payout'
          AND processing_result = 'payout_webhook_recorded'
        "#,
    )
    .fetch_one(&harness.db)
    .await
    .unwrap();
    assert!(payout_count >= 1);
}

#[tokio::test]
async fn expired_payment_webhook_finalizes_without_pending_credit() {
    let harness = build_harness().await;
    let owner = insert_user(&harness.db, "user").await;
    let store = insert_store(&harness.db, owner.id, false).await;
    let payment = insert_pending_payment(&harness.db, &store, 44_000, Some("expired-ref")).await;
    let app = backend::modules::payments::webhook_routes(harness.state.clone())
        .with_state(harness.state.clone());
    let mut receiver = harness.state.realtime_service.subscribe();

    let response = app
        .oneshot(webhook_request(payment_webhook_payload(
            &harness.merchant_id,
            &payment,
            &store.provider_terminal_id,
            "expired",
            payment.custom_ref.as_deref(),
        )))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let event = tokio::time::timeout(Duration::from_secs(1), receiver.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(event.event_type, "payment.updated");

    let payment_status = sqlx::query_scalar::<_, String>("SELECT status FROM payments WHERE id = $1")
        .bind(payment.id)
        .fetch_one(&harness.db)
        .await
        .unwrap();
    assert_eq!(payment_status, "expired");

    let summary_exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM store_balance_summaries WHERE store_id = $1)",
    )
    .bind(store.id)
    .fetch_one(&harness.db)
    .await
    .unwrap();
    assert!(!summary_exists);

    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM store_balance_ledger_entries WHERE related_id = $1",
        )
        .bind(payment.id)
        .fetch_one(&harness.db)
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM platform_ledger_entries WHERE related_id = $1",
        )
        .bind(payment.id)
        .fetch_one(&harness.db)
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM user_notifications WHERE user_id = $1 AND related_id = $2",
        )
        .bind(owner.id)
        .bind(payment.id)
        .fetch_one(&harness.db)
        .await
        .unwrap(),
        1
    );
}

#[tokio::test]
async fn dashboard_payment_routes_and_notification_routes_are_scoped_and_mark_read_requires_csrf() {
    let harness = build_harness().await;
    let owner = insert_user(&harness.db, "user").await;
    let other_user = insert_user(&harness.db, "user").await;
    let store = insert_store(&harness.db, owner.id, true).await;
    let payment = insert_pending_payment(&harness.db, &store, 18_000, Some("notify-ref")).await;
    let webhook_app = backend::modules::payments::webhook_routes(harness.state.clone())
        .with_state(harness.state.clone());
    let dashboard_app = backend::modules::payments::dashboard_routes(harness.state.clone())
        .with_state(harness.state.clone());
    let notifications_app = backend::modules::notifications::routes(harness.state.clone())
        .layer(middleware::from_fn(csrf_middleware))
        .with_state(harness.state.clone());

    insert_membership(&harness.db, store.id, owner.id, StoreRole::Owner).await;

    let webhook_response = webhook_app
        .oneshot(webhook_request(payment_webhook_payload(
            &harness.merchant_id,
            &payment,
            &store.provider_terminal_id,
            "success",
            payment.custom_ref.as_deref(),
        )))
        .await
        .unwrap();
    assert_eq!(webhook_response.status(), StatusCode::OK);

    let mut list_request = Request::builder()
        .method("GET")
        .uri("/")
        .body(Body::empty())
        .unwrap();
    list_request
        .extensions_mut()
        .insert(session_context(&owner, "csrf-owner"));
    let list_response = dashboard_app.clone().oneshot(list_request).await.unwrap();
    assert_eq!(list_response.status(), StatusCode::OK);
    let list_body = to_bytes(list_response.into_body(), usize::MAX).await.unwrap();
    let list_payload: Value = serde_json::from_slice(&list_body).unwrap();
    assert_eq!(list_payload["total"], json!(1));
    assert_eq!(list_payload["payments"][0]["id"], json!(payment.id.to_string()));

    let mut detail_request = Request::builder()
        .method("GET")
        .uri(format!("/{}", payment.id))
        .body(Body::empty())
        .unwrap();
    detail_request
        .extensions_mut()
        .insert(session_context(&owner, "csrf-owner"));
    let detail_response = dashboard_app.clone().oneshot(detail_request).await.unwrap();
    assert_eq!(detail_response.status(), StatusCode::OK);

    let mut blocked_detail_request = Request::builder()
        .method("GET")
        .uri(format!("/{}", payment.id))
        .body(Body::empty())
        .unwrap();
    blocked_detail_request
        .extensions_mut()
        .insert(session_context(&other_user, "csrf-other"));
    let blocked_detail_response = dashboard_app
        .clone()
        .oneshot(blocked_detail_request)
        .await
        .unwrap();
    assert_eq!(blocked_detail_response.status(), StatusCode::NOT_FOUND);

    let mut notifications_request = Request::builder()
        .method("GET")
        .uri("/")
        .body(Body::empty())
        .unwrap();
    notifications_request
        .extensions_mut()
        .insert(session_context(&owner, "csrf-owner"));
    let notifications_response = notifications_app
        .clone()
        .oneshot(notifications_request)
        .await
        .unwrap();
    assert_eq!(notifications_response.status(), StatusCode::OK);
    let notifications_body = to_bytes(notifications_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let notifications_payload: Value = serde_json::from_slice(&notifications_body).unwrap();
    assert_eq!(notifications_payload["unread_count"], json!(1));
    let notification_id = Uuid::parse_str(
        notifications_payload["notifications"][0]["id"]
            .as_str()
            .unwrap(),
    )
    .unwrap();

    let mut missing_csrf_request = Request::builder()
        .method("POST")
        .uri(format!("/{notification_id}/read"))
        .body(Body::empty())
        .unwrap();
    missing_csrf_request
        .extensions_mut()
        .insert(session_context(&owner, "csrf-owner"));
    let missing_csrf_response = notifications_app
        .clone()
        .oneshot(missing_csrf_request)
        .await
        .unwrap();
    assert_eq!(missing_csrf_response.status(), StatusCode::FORBIDDEN);

    let mut valid_mark_read_request = Request::builder()
        .method("POST")
        .uri(format!("/{notification_id}/read"))
        .header("X-CSRF-Token", "csrf-owner")
        .body(Body::empty())
        .unwrap();
    valid_mark_read_request
        .extensions_mut()
        .insert(session_context(&owner, "csrf-owner"));
    let valid_mark_read_response = notifications_app
        .clone()
        .oneshot(valid_mark_read_request)
        .await
        .unwrap();
    assert_eq!(valid_mark_read_response.status(), StatusCode::NO_CONTENT);

    let mut repeated_mark_read_request = Request::builder()
        .method("POST")
        .uri(format!("/{notification_id}/read"))
        .header("X-CSRF-Token", "csrf-owner")
        .body(Body::empty())
        .unwrap();
    repeated_mark_read_request
        .extensions_mut()
        .insert(session_context(&owner, "csrf-owner"));
    let repeated_mark_read_response = notifications_app
        .clone()
        .oneshot(repeated_mark_read_request)
        .await
        .unwrap();
    assert_eq!(repeated_mark_read_response.status(), StatusCode::NO_CONTENT);

    let unread_after = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM user_notifications WHERE user_id = $1 AND status = 'unread'",
    )
    .bind(owner.id)
    .fetch_one(&harness.db)
    .await
    .unwrap();
    assert_eq!(unread_after, 0);

    let mut blocked_mark_read_request = Request::builder()
        .method("POST")
        .uri(format!("/{notification_id}/read"))
        .header("X-CSRF-Token", "csrf-other")
        .body(Body::empty())
        .unwrap();
    blocked_mark_read_request
        .extensions_mut()
        .insert(session_context(&other_user, "csrf-other"));
    let blocked_mark_read_response = notifications_app
        .clone()
        .oneshot(blocked_mark_read_request)
        .await
        .unwrap();
    assert_eq!(blocked_mark_read_response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn realtime_stream_requires_auth_and_opens_sse_response() {
    let harness = build_harness().await;
    let owner = insert_user(&harness.db, "user").await;
    let store = insert_store(&harness.db, owner.id, false).await;
    let _payment = insert_pending_payment(&harness.db, &store, 10_000, Some("stream-ref")).await;
    let realtime_app = backend::modules::realtime::routes(harness.state.clone())
        .with_state(harness.state.clone());

    let unauthorized = realtime_app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/stream")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

    let mut authorized_request = Request::builder()
        .method("GET")
        .uri("/stream")
        .body(Body::empty())
        .unwrap();
    authorized_request
        .extensions_mut()
        .insert(session_context(&owner, "csrf-owner"));
    let authorized = realtime_app.clone().oneshot(authorized_request).await.unwrap();
    assert_eq!(authorized.status(), StatusCode::OK);
    let content_type = authorized
        .headers()
        .get("content-type")
        .and_then(|header| header.to_str().ok())
        .unwrap_or_default()
        .to_string();
    assert!(content_type.starts_with("text/event-stream"));
}
