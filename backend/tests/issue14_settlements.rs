use std::sync::Arc;
use std::time::Duration;

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
use backend::modules::settlements::domain::entity::SettlementRequest;
use backend::modules::settlements::infrastructure::repository::SqlxSettlementRepository;
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
use backend::shared::auth::{AuthenticatedUser, PlatformRole, StoreRole};
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

    async fn list_active_tokens(&self, _store_id: Uuid) -> anyhow::Result<Vec<StoreApiTokenRecord>> {
        Ok(vec![])
    }

    async fn insert_token(&self, _token: NewStoreApiTokenRecord) -> anyhow::Result<StoreApiTokenRecord> {
        Err(anyhow!("store tokens are not used in settlement tests"))
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
        Err(AppError::Internal(anyhow!("not used in settlement tests")))
    }

    async fn check_payment_status(
        &self,
        _request: CheckPaymentStatusRequest,
    ) -> Result<CheckedPaymentStatus, AppError> {
        Err(AppError::Internal(anyhow!("not used in settlement tests")))
    }

    async fn inquiry_bank(&self, _request: InquiryBankRequest) -> Result<InquiryBankResult, AppError> {
        Err(AppError::Internal(anyhow!("not used in settlement tests")))
    }

    async fn transfer(&self, _request: TransferRequest) -> Result<TransferResult, AppError> {
        Err(AppError::Internal(anyhow!("not used in settlement tests")))
    }

    async fn check_disbursement_status(
        &self,
        _request: CheckDisbursementStatusRequest,
    ) -> Result<CheckedDisbursementStatus, AppError> {
        Err(AppError::Internal(anyhow!("not used in settlement tests")))
    }

    async fn get_balance(&self, _request: GetBalanceRequest) -> Result<ProviderBalanceSnapshot, AppError> {
        Err(AppError::Internal(anyhow!("not used in settlement tests")))
    }
}

#[derive(Clone)]
struct TestHarness {
    state: AppState,
    db: PgPool,
}

#[derive(Debug, Clone)]
struct SeededUser {
    id: Uuid,
    role: &'static str,
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
        Arc::new(NoopProvider),
    ));
    let payment_idempotency_service =
        Arc::new(PaymentIdempotencyService::new(payment_repository));
    let notification_service = Arc::new(NotificationService::new(notification_repository));
    let realtime_service = Arc::new(RealtimeService::new(64));
    let settlement_service = Arc::new(SettlementService::new(settlement_repository));
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
        store_service,
        store_token_service,
        support_service,
        user_service,
    };

    TestHarness { state, db }
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
    .bind(format!("{role}-{}@issue14.test", id.simple()))
    .bind(role)
    .bind(now)
    .execute(db)
    .await
    .unwrap();

    SeededUser { id, role }
}

async fn insert_store(db: &PgPool, owner_user_id: Uuid) -> SeededStore {
    let id = Uuid::new_v4();
    let name = format!("Store {}", &id.simple().to_string()[..8]);
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
    .bind(name.clone())
    .bind(format!("store-{}", id.simple()))
    .bind(format!("terminal-{}", &id.simple().to_string()[..12]))
    .bind(now)
    .execute(db)
    .await
    .unwrap();

    insert_membership(db, id, owner_user_id, StoreRole::Owner).await;

    let _ = name;
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

async fn insert_balance_summary(
    db: &PgPool,
    store_id: Uuid,
    pending: i64,
    settled: i64,
    reserved: i64,
) {
    sqlx::query(
        r#"
        INSERT INTO store_balance_summaries (
            store_id,
            pending_balance,
            settled_balance,
            reserved_settled_balance,
            updated_at
        )
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (store_id)
        DO UPDATE SET
            pending_balance = EXCLUDED.pending_balance,
            settled_balance = EXCLUDED.settled_balance,
            reserved_settled_balance = EXCLUDED.reserved_settled_balance,
            updated_at = EXCLUDED.updated_at
        "#,
    )
    .bind(store_id)
    .bind(pending)
    .bind(settled)
    .bind(reserved)
    .bind(Utc::now())
    .execute(db)
    .await
    .unwrap();
}

fn session_context(user: &SeededUser, csrf_token: &str) -> SessionContext {
    SessionContext {
        session_id: Uuid::new_v4(),
        user: UserProfile {
            id: user.id,
            name: format!("User {}", user.role),
            email: format!("{}@issue14.test", user.id.simple()),
            role: user.role.into(),
            status: "active".into(),
        },
        csrf_token: csrf_token.into(),
    }
}

fn settlement_app(state: AppState) -> Router {
    Router::new()
        .nest("/api/v1/dev", backend::modules::settlements::routes(state.clone()))
        .nest("/api/v1/stores", backend::modules::balances::store_routes(state.clone()))
        .layer(middleware::from_fn(csrf_middleware))
        .with_state(state)
}

fn settlement_request(
    store_id: Uuid,
    amount: i64,
    notes: Option<&str>,
    csrf_token: Option<&str>,
) -> Request<Body> {
    let mut builder = Request::builder()
        .method("POST")
        .uri("/api/v1/dev/settlements")
        .header("content-type", "application/json");

    if let Some(token) = csrf_token {
        builder = builder.header("X-CSRF-Token", token);
    }

    builder
        .body(Body::from(
            json!({
                "store_id": store_id,
                "amount": amount,
                "notes": notes,
            })
            .to_string(),
        ))
        .unwrap()
}

fn dev_actor(user_id: Uuid) -> AuthenticatedUser {
    AuthenticatedUser {
        user_id,
        platform_role: PlatformRole::Dev,
        memberships: Default::default(),
    }
}

#[tokio::test]
async fn dev_can_settle_pending_balance_and_route_writes_all_side_effects() {
    let harness = build_harness().await;
    let dev = insert_user(&harness.db, "dev").await;
    let owner = insert_user(&harness.db, "user").await;
    let second_owner = insert_user(&harness.db, "user").await;
    let store = insert_store(&harness.db, owner.id).await;
    insert_membership(&harness.db, store.id, second_owner.id, StoreRole::Owner).await;
    insert_balance_summary(&harness.db, store.id, 120_000, 45_000, 5_000).await;
    let app = settlement_app(harness.state.clone());
    let mut receiver = harness.state.realtime_service.subscribe();

    let mut request = settlement_request(store.id, 30_000, Some("  Manual dev settlement  "), Some("csrf-dev"));
    request
        .extensions_mut()
        .insert(session_context(&dev, "csrf-dev"));

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();
    let settlement_id = Uuid::parse_str(payload["settlement"]["id"].as_str().unwrap()).unwrap();
    assert_eq!(payload["settlement"]["status"], json!("processed"));
    assert_eq!(payload["settlement"]["amount"], json!(30_000));
    assert_eq!(payload["settlement"]["notes"], json!("Manual dev settlement"));
    assert_eq!(payload["balance"]["pending_balance"], json!(90_000));
    assert_eq!(payload["balance"]["settled_balance"], json!(75_000));
    assert_eq!(payload["balance"]["withdrawable_balance"], json!(70_000));

    let balance_event = tokio::time::timeout(Duration::from_secs(1), receiver.recv())
        .await
        .unwrap()
        .unwrap();
    let notification_event = tokio::time::timeout(Duration::from_secs(1), receiver.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(balance_event.event_type, "store.balance.updated");
    assert_eq!(notification_event.event_type, "notification.created");

    let summary = sqlx::query(
        r#"
        SELECT pending_balance, settled_balance, reserved_settled_balance
        FROM store_balance_summaries
        WHERE store_id = $1
        "#,
    )
    .bind(store.id)
    .fetch_one(&harness.db)
    .await
    .unwrap();
    assert_eq!(summary.get::<i64, _>("pending_balance"), 90_000);
    assert_eq!(summary.get::<i64, _>("settled_balance"), 75_000);
    assert_eq!(summary.get::<i64, _>("reserved_settled_balance"), 5_000);

    let settlement_row = sqlx::query(
        r#"
        SELECT store_id, amount, status, processed_by_user_id, notes
        FROM store_balance_settlements
        WHERE id = $1
        "#,
    )
    .bind(settlement_id)
    .fetch_one(&harness.db)
    .await
    .unwrap();
    assert_eq!(settlement_row.get::<Uuid, _>("store_id"), store.id);
    assert_eq!(settlement_row.get::<i64, _>("amount"), 30_000);
    assert_eq!(settlement_row.get::<String, _>("status"), "processed");
    assert_eq!(settlement_row.get::<Uuid, _>("processed_by_user_id"), dev.id);
    assert_eq!(
        settlement_row.get::<Option<String>, _>("notes").as_deref(),
        Some("Manual dev settlement")
    );

    let ledger_rows = sqlx::query(
        r#"
        SELECT direction, balance_bucket, amount, related_type, related_id, entry_type
        FROM store_balance_ledger_entries
        WHERE related_id = $1
        ORDER BY balance_bucket ASC
        "#,
    )
    .bind(settlement_id)
    .fetch_all(&harness.db)
    .await
    .unwrap();
    assert_eq!(ledger_rows.len(), 2);
    assert_eq!(
        ledger_rows
            .iter()
            .filter(|row| row.get::<String, _>("balance_bucket") == "pending")
            .count(),
        1
    );
    assert_eq!(
        ledger_rows
            .iter()
            .filter(|row| row.get::<String, _>("direction") == "debit")
            .count(),
        1
    );
    assert_eq!(
        ledger_rows
            .iter()
            .filter(|row| row.get::<String, _>("balance_bucket") == "settled")
            .count(),
        1
    );
    assert!(ledger_rows
        .iter()
        .all(|row| row.get::<i64, _>("amount") == 30_000));
    assert!(ledger_rows.iter().all(|row| row.get::<String, _>("related_type") == "settlement"));
    assert!(ledger_rows.iter().all(|row| row.get::<String, _>("entry_type") == "settlement_move_pending_to_settled"));

    let notification_rows = sqlx::query(
        r#"
        SELECT user_id, type, related_type, related_id, status
        FROM user_notifications
        WHERE related_id = $1
        ORDER BY user_id ASC
        "#,
    )
    .bind(settlement_id)
    .fetch_all(&harness.db)
    .await
    .unwrap();
    assert_eq!(notification_rows.len(), 2);
    assert!(notification_rows
        .iter()
        .any(|row| row.get::<Uuid, _>("user_id") == owner.id));
    assert!(notification_rows
        .iter()
        .any(|row| row.get::<Uuid, _>("user_id") == second_owner.id));
    assert!(notification_rows
        .iter()
        .all(|row| row.get::<String, _>("type") == "settlement_processed"));
    assert!(notification_rows
        .iter()
        .all(|row| row.get::<Option<String>, _>("related_type").as_deref() == Some("settlement")));
    assert!(notification_rows
        .iter()
        .all(|row| row.get::<String, _>("status") == "unread"));

    let audit_row = sqlx::query(
        r#"
        SELECT actor_user_id, action, target_type, target_id, payload_json
        FROM audit_logs
        WHERE target_id = $1
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(settlement_id)
    .fetch_one(&harness.db)
    .await
    .unwrap();
    assert_eq!(audit_row.get::<Option<Uuid>, _>("actor_user_id"), Some(dev.id));
    assert_eq!(audit_row.get::<String, _>("action"), "settlement.processed");
    assert_eq!(audit_row.get::<Option<String>, _>("target_type").as_deref(), Some("store_balance_settlement"));
    let audit_payload = audit_row.get::<Value, _>("payload_json");
    assert_eq!(audit_payload["store_id"], json!(store.id));
    assert_eq!(audit_payload["amount"], json!(30_000));
    assert_eq!(audit_payload["pending_balance"], json!(90_000));
    assert_eq!(audit_payload["settled_balance"], json!(75_000));

    let mut balance_request = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/stores/{}/balances", store.id))
        .body(Body::empty())
        .unwrap();
    balance_request
        .extensions_mut()
        .insert(session_context(&owner, "csrf-owner"));

    let balance_response = app.clone().oneshot(balance_request).await.unwrap();
    assert_eq!(balance_response.status(), StatusCode::OK);
    let balance_body = to_bytes(balance_response.into_body(), usize::MAX).await.unwrap();
    let balance_payload: Value = serde_json::from_slice(&balance_body).unwrap();
    assert_eq!(balance_payload["balance"]["pending_balance"], json!(90_000));
    assert_eq!(balance_payload["balance"]["settled_balance"], json!(75_000));
    assert_eq!(balance_payload["balance"]["withdrawable_balance"], json!(70_000));
}

#[tokio::test]
async fn settlement_route_rejects_invalid_amounts_and_oversettlement() {
    let harness = build_harness().await;
    let dev = insert_user(&harness.db, "dev").await;
    let owner = insert_user(&harness.db, "user").await;
    let store = insert_store(&harness.db, owner.id).await;
    insert_balance_summary(&harness.db, store.id, 10_000, 0, 0).await;
    let app = settlement_app(harness.state.clone());

    let mut zero_request = settlement_request(store.id, 0, None, Some("csrf-dev"));
    zero_request
        .extensions_mut()
        .insert(session_context(&dev, "csrf-dev"));
    let zero_response = app.clone().oneshot(zero_request).await.unwrap();
    assert_eq!(zero_response.status(), StatusCode::BAD_REQUEST);

    let mut overspend_request = settlement_request(store.id, 10_001, None, Some("csrf-dev"));
    overspend_request
        .extensions_mut()
        .insert(session_context(&dev, "csrf-dev"));
    let overspend_response = app.clone().oneshot(overspend_request).await.unwrap();
    assert_eq!(overspend_response.status(), StatusCode::CONFLICT);
    let overspend_body = to_bytes(overspend_response.into_body(), usize::MAX).await.unwrap();
    let overspend_payload: Value = serde_json::from_slice(&overspend_body).unwrap();
    assert!(overspend_payload["error"]["message"]
        .as_str()
        .unwrap()
        .contains("exceeds pending balance"));
}

#[tokio::test]
async fn settlement_route_requires_dev_role_and_csrf() {
    let harness = build_harness().await;
    let owner = insert_user(&harness.db, "user").await;
    let superadmin = insert_user(&harness.db, "superadmin").await;
    let store = insert_store(&harness.db, owner.id).await;
    insert_balance_summary(&harness.db, store.id, 5_000, 1_000, 0).await;
    let app = settlement_app(harness.state.clone());

    let mut missing_csrf_request = settlement_request(store.id, 1_000, None, None);
    missing_csrf_request
        .extensions_mut()
        .insert(session_context(&superadmin, "csrf-superadmin"));
    let missing_csrf_response = app.clone().oneshot(missing_csrf_request).await.unwrap();
    assert_eq!(missing_csrf_response.status(), StatusCode::FORBIDDEN);

    let mut forbidden_request = settlement_request(store.id, 1_000, None, Some("csrf-superadmin"));
    forbidden_request
        .extensions_mut()
        .insert(session_context(&superadmin, "csrf-superadmin"));
    let forbidden_response = app.oneshot(forbidden_request).await.unwrap();
    assert_eq!(forbidden_response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn concurrency_guard_prevents_oversettlement() {
    let harness = build_harness().await;
    let dev = insert_user(&harness.db, "dev").await;
    let owner = insert_user(&harness.db, "user").await;
    let store = insert_store(&harness.db, owner.id).await;
    insert_balance_summary(&harness.db, store.id, 100_000, 0, 0).await;

    let actor = dev_actor(dev.id);
    let service = harness.state.settlement_service.clone();
    let store_id = store.id;

    let first_actor = actor.clone();
    let first_service = service.clone();
    let first = tokio::spawn(async move {
        first_service
            .process_settlement(
                SettlementRequest {
                    store_id,
                    amount: 75_000,
                    notes: Some("first".into()),
                },
                &first_actor,
            )
            .await
    });

    let second_actor = actor.clone();
    let second_service = service.clone();
    let second = tokio::spawn(async move {
        second_service
            .process_settlement(
                SettlementRequest {
                    store_id,
                    amount: 75_000,
                    notes: Some("second".into()),
                },
                &second_actor,
            )
            .await
    });

    let first_result = first.await.unwrap();
    let second_result = second.await.unwrap();
    assert_eq!(
        [first_result.is_ok(), second_result.is_ok()]
            .into_iter()
            .filter(|value| *value)
            .count(),
        1
    );
    assert_eq!(
        [matches!(first_result, Err(AppError::Conflict(_))), matches!(second_result, Err(AppError::Conflict(_)))]
            .into_iter()
            .filter(|value| *value)
            .count(),
        1
    );

    let summary = sqlx::query(
        r#"
        SELECT pending_balance, settled_balance
        FROM store_balance_summaries
        WHERE store_id = $1
        "#,
    )
    .bind(store.id)
    .fetch_one(&harness.db)
    .await
    .unwrap();
    assert_eq!(summary.get::<i64, _>("pending_balance"), 25_000);
    assert_eq!(summary.get::<i64, _>("settled_balance"), 75_000);

    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM store_balance_settlements WHERE store_id = $1",
        )
        .bind(store.id)
        .fetch_one(&harness.db)
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM store_balance_ledger_entries WHERE store_id = $1 AND related_type = 'settlement'",
        )
        .bind(store.id)
        .fetch_one(&harness.db)
        .await
        .unwrap(),
        2
    );
}
