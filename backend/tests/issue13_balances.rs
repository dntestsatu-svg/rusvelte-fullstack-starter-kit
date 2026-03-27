use std::sync::Arc;

use anyhow::anyhow;
use async_trait::async_trait;
use axum::extract::{Path, State};
use axum::Extension;
use backend::bootstrap::config::Config;
use backend::bootstrap::state::AppState;
use backend::infrastructure::redis::RedisPool;
use backend::infrastructure::security::captcha::NoOpCaptchaVerifier;
use backend::modules::auth::application::dto::{SessionContext, UserProfile};
use backend::modules::auth::application::service::AuthService;
use backend::modules::auth::domain::repository::AuthRepository;
use backend::modules::auth::domain::session::Session;
use backend::modules::auth::domain::user::AuthUser;
use backend::modules::balances::application::service::StoreBalanceService;
use backend::modules::balances::domain::entity::{
    BalanceBucket, BalanceSummaryDelta, LedgerDirection, NewStoreBalanceLedgerEntry,
    StoreBalanceEntryType, StoreBalanceSummary,
};
use backend::modules::balances::infrastructure::repository::SqlxStoreBalanceRepository;
use backend::modules::payments::application::idempotency::PaymentIdempotencyService;
use backend::modules::payments::application::provider::{
    CheckDisbursementStatusRequest, CheckPaymentStatusRequest, CheckedDisbursementStatus,
    CheckedPaymentStatus, GenerateQrisRequest, GeneratedQris, GetBalanceRequest,
    InquiryBankRequest, InquiryBankResult, PaymentProviderGateway, ProviderBalanceSnapshot,
    TransferRequest, TransferResult,
};
use backend::modules::payments::application::service::PaymentService;
use backend::modules::payments::domain::entity::{
    PaymentStatus, PAYMENT_PLATFORM_FEE_BPS, PAYMENT_PROVIDER_NAME,
};
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
use backend::shared::error::AppError;
use bb8::Pool;
use bb8_redis::RedisConnectionManager;
use chrono::Utc;
use sqlx::{postgres::PgPoolOptions, PgPool};
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
        Err(anyhow!("store tokens are not used in balance tests"))
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
struct MockProvider {
    snapshot: ProviderBalanceSnapshot,
}

#[async_trait]
impl PaymentProviderGateway for MockProvider {
    async fn generate_qris(&self, _request: GenerateQrisRequest) -> Result<GeneratedQris, AppError> {
        Err(AppError::Internal(anyhow!("generate_qris not used in balance tests")))
    }

    async fn check_payment_status(
        &self,
        _request: CheckPaymentStatusRequest,
    ) -> Result<CheckedPaymentStatus, AppError> {
        Err(AppError::Internal(anyhow!("check_payment_status not used in balance tests")))
    }

    async fn inquiry_bank(
        &self,
        _request: InquiryBankRequest,
    ) -> Result<InquiryBankResult, AppError> {
        Err(AppError::Internal(anyhow!("inquiry_bank not used in balance tests")))
    }

    async fn transfer(&self, _request: TransferRequest) -> Result<TransferResult, AppError> {
        Err(AppError::Internal(anyhow!("transfer not used in balance tests")))
    }

    async fn check_disbursement_status(
        &self,
        _request: CheckDisbursementStatusRequest,
    ) -> Result<CheckedDisbursementStatus, AppError> {
        Err(AppError::Internal(anyhow!("check_disbursement_status not used in balance tests")))
    }

    async fn get_balance(
        &self,
        _request: GetBalanceRequest,
    ) -> Result<ProviderBalanceSnapshot, AppError> {
        Ok(self.snapshot.clone())
    }
}

#[derive(Clone)]
struct TestHarness {
    state: AppState,
    db: PgPool,
}

fn build_redis_pool(config: &Config) -> RedisPool {
    let manager = RedisConnectionManager::new(config.redis_url.clone()).unwrap();
    Pool::builder().build_unchecked(manager)
}

async fn build_harness(provider: Arc<dyn PaymentProviderGateway>) -> TestHarness {
    let base_config = Config::from_env().unwrap();
    let db = PgPoolOptions::new()
        .max_connections(5)
        .connect(&base_config.database_url)
        .await
        .unwrap();
    let redis = build_redis_pool(&base_config);

    let auth_service = Arc::new(AuthService::new(
        Arc::new(MockAuthRepository),
        Arc::new(NoOpCaptchaVerifier),
        redis.clone(),
    ));
    let audit_repository = Arc::new(NoopAuditRepository);
    let user_repository = Arc::new(SqlxUserRepository::new(db.clone()));
    let store_repository = Arc::new(SqlxStoreRepository::new(db.clone()));
    let balance_repository = Arc::new(SqlxStoreBalanceRepository::new(db.clone()));
    let payment_repository = Arc::new(SqlxPaymentRepository::new(db.clone()));
    let notification_repository = Arc::new(
        backend::modules::notifications::infrastructure::repository::SqlxNotificationRepository::new(
            db.clone(),
        ),
    );

    let user_service = Arc::new(UserService::new(user_repository.clone(), audit_repository.clone()));
    let store_service = Arc::new(StoreService::new(
        store_repository,
        user_repository,
        audit_repository.clone(),
    ));
    let balance_service = Arc::new(StoreBalanceService::new(balance_repository));
    let payment_service = Arc::new(PaymentService::new(payment_repository.clone(), provider));
    let payment_idempotency_service = Arc::new(PaymentIdempotencyService::new(payment_repository));
    let notification_service = Arc::new(backend::modules::notifications::application::service::NotificationService::new(notification_repository));
    let realtime_service = Arc::new(RealtimeService::new(64));
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
        store_service,
        store_token_service,
        support_service,
        user_service,
    };

    TestHarness { state, db }
}

async fn insert_user(db: &PgPool, role: &str) -> Uuid {
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
    .bind(format!("{role}-{}@issue13.test", id.simple()))
    .bind(role)
    .bind(now)
    .execute(db)
    .await
    .unwrap();

    id
}

async fn insert_store(db: &PgPool, owner_user_id: Uuid) -> Uuid {
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
    .bind(format!("Store {}", &id.simple().to_string()[..8]))
    .bind(format!("store-{}", id.simple()))
    .bind(format!("terminal-{}", &id.simple().to_string()[..12]))
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
        VALUES ($1, $2, $3, 'owner', 'active', $3, $4, $4)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(id)
    .bind(owner_user_id)
    .bind(now)
    .execute(db)
    .await
    .unwrap();

    id
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

async fn insert_payment(db: &PgPool, store_id: Uuid, status: PaymentStatus) {
    let now = Utc::now();
    sqlx::query(
        r#"
        INSERT INTO payments (
            id,
            store_id,
            created_by_user_id,
            provider_name,
            provider_terminal_id,
            gross_amount,
            platform_tx_fee_bps,
            platform_tx_fee_amount,
            store_pending_credit_amount,
            status,
            created_at,
            updated_at
        )
        VALUES ($1, $2, NULL, $3, $4, 10000, $5, 300, 9700, $6, $7, $7)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(store_id)
    .bind(PAYMENT_PROVIDER_NAME)
    .bind("terminal-issue13")
    .bind(PAYMENT_PLATFORM_FEE_BPS)
    .bind(status.to_string())
    .bind(now)
    .execute(db)
    .await
    .unwrap();
}

fn session_context(user_id: Uuid, role: &str) -> SessionContext {
    SessionContext {
        session_id: Uuid::new_v4(),
        user: UserProfile {
            id: user_id,
            name: "Session User".into(),
            email: "session@example.com".into(),
            role: role.to_string(),
            status: "active".into(),
        },
        csrf_token: "csrf-test".into(),
    }
}

#[tokio::test]
async fn withdrawable_balance_formula_is_consistent() {
    let summary = StoreBalanceSummary {
        store_id: Uuid::new_v4(),
        pending_balance: 10_000,
        settled_balance: 25_000,
        reserved_settled_balance: 4_000,
        updated_at: Utc::now(),
    };

    assert_eq!(summary.withdrawable_balance(), 21_000);
}

#[tokio::test]
async fn store_balance_api_returns_snapshot_and_withdrawable() {
    let provider = Arc::new(MockProvider {
        snapshot: ProviderBalanceSnapshot {
            provider_pending_balance: 500,
            provider_settle_balance: 600,
        },
    });
    let harness = build_harness(provider).await;
    let user_id = insert_user(&harness.db, "user").await;
    let store_id = insert_store(&harness.db, user_id).await;
    insert_balance_summary(&harness.db, store_id, 12_000, 8_000, 3_000).await;

    let response = backend::modules::balances::interfaces::http::handlers::get_store_balance(
        State(harness.state.clone()),
        Some(Extension(session_context(user_id, "user"))),
        Path(store_id),
    )
    .await
    .unwrap();

    let backend::modules::balances::interfaces::http::handlers::StoreBalanceResponse { balance } =
        response.0;
    assert_eq!(balance.pending_balance, 12_000);
    assert_eq!(balance.settled_balance, 8_000);
    assert_eq!(balance.reserved_settled_balance, 3_000);
    assert_eq!(balance.withdrawable_balance, 5_000);
}

#[tokio::test]
async fn store_balance_api_is_scoped_to_membership() {
    let provider = Arc::new(MockProvider {
        snapshot: ProviderBalanceSnapshot {
            provider_pending_balance: 10,
            provider_settle_balance: 20,
        },
    });
    let harness = build_harness(provider).await;
    let owner_id = insert_user(&harness.db, "user").await;
    let outsider_id = insert_user(&harness.db, "user").await;
    let store_id = insert_store(&harness.db, owner_id).await;
    insert_balance_summary(&harness.db, store_id, 1_000, 0, 0).await;

    let result = backend::modules::balances::interfaces::http::handlers::get_store_balance(
        State(harness.state.clone()),
        Some(Extension(session_context(outsider_id, "user"))),
        Path(store_id),
    )
    .await;

    assert!(matches!(result, Err(AppError::NotFound(_))));
}

#[tokio::test]
async fn dashboard_payment_distribution_counts_final_statuses_only() {
    let provider = Arc::new(MockProvider {
        snapshot: ProviderBalanceSnapshot {
            provider_pending_balance: 0,
            provider_settle_balance: 0,
        },
    });
    let harness = build_harness(provider).await;
    let user_id = insert_user(&harness.db, "admin").await;
    let store_id = insert_store(&harness.db, user_id).await;

    insert_payment(&harness.db, store_id, PaymentStatus::Success).await;
    insert_payment(&harness.db, store_id, PaymentStatus::Success).await;
    insert_payment(&harness.db, store_id, PaymentStatus::Failed).await;
    insert_payment(&harness.db, store_id, PaymentStatus::Expired).await;
    insert_payment(&harness.db, store_id, PaymentStatus::Pending).await;

    let response = backend::modules::payments::interfaces::http::handlers::get_dashboard_payment_distribution(
        State(harness.state.clone()),
        Some(Extension(session_context(user_id, "admin"))),
    )
    .await
    .unwrap();

    assert_eq!(response.distribution.success, 2);
    assert_eq!(response.distribution.failed, 1);
    assert_eq!(response.distribution.expired, 1);
}

#[tokio::test]
async fn provider_balance_is_dev_only() {
    let provider = Arc::new(MockProvider {
        snapshot: ProviderBalanceSnapshot {
            provider_pending_balance: 111,
            provider_settle_balance: 222,
        },
    });
    let harness = build_harness(provider).await;
    let dev_id = insert_user(&harness.db, "dev").await;
    let superadmin_id = insert_user(&harness.db, "superadmin").await;

    let response = backend::modules::balances::interfaces::http::handlers::get_provider_balance(
        State(harness.state.clone()),
        Some(Extension(session_context(dev_id, "dev"))),
    )
    .await
    .unwrap();

    assert_eq!(response.provider_pending_balance, 111);
    assert_eq!(response.provider_settle_balance, 222);

    let forbidden = backend::modules::balances::interfaces::http::handlers::get_provider_balance(
        State(harness.state.clone()),
        Some(Extension(session_context(superadmin_id, "superadmin"))),
    )
    .await;

    assert!(matches!(forbidden, Err(AppError::Forbidden(_))));
}

#[tokio::test]
async fn ledger_entries_are_append_only_via_service() {
    let provider = Arc::new(MockProvider {
        snapshot: ProviderBalanceSnapshot {
            provider_pending_balance: 0,
            provider_settle_balance: 0,
        },
    });
    let harness = build_harness(provider).await;
    let user_id = insert_user(&harness.db, "user").await;
    let store_id = insert_store(&harness.db, user_id).await;
    let now = Utc::now();

    let entry = harness
        .state
        .balance_service
        .append_ledger_entry(NewStoreBalanceLedgerEntry {
            store_id,
            related_type: "manual".into(),
            related_id: None,
            entry_type: StoreBalanceEntryType::ManualAdjustment,
            amount: 5_000,
            direction: LedgerDirection::Credit,
            balance_bucket: BalanceBucket::Pending,
            description: Some("Manual credit".into()),
            created_at: now,
        })
        .await
        .unwrap();

    assert_eq!(entry.amount, 5_000);
    assert_eq!(entry.balance_bucket, BalanceBucket::Pending);
}

#[tokio::test]
async fn balance_summary_delta_updates_pending_and_settled_fields() {
    let provider = Arc::new(MockProvider {
        snapshot: ProviderBalanceSnapshot {
            provider_pending_balance: 0,
            provider_settle_balance: 0,
        },
    });
    let harness = build_harness(provider).await;
    let user_id = insert_user(&harness.db, "user").await;
    let store_id = insert_store(&harness.db, user_id).await;

    let summary = harness
        .state
        .balance_service
        .apply_summary_delta(
            store_id,
            BalanceSummaryDelta {
                pending_delta: 2_500,
                settled_delta: 1_000,
                reserved_delta: 0,
            },
            Utc::now(),
        )
        .await
        .unwrap();

    assert_eq!(summary.pending_balance, 2_500);
    assert_eq!(summary.settled_balance, 1_000);
}
