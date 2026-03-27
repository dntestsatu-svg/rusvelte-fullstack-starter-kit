use axum::{
    middleware,
    routing::{get, post},
    Router,
};

use crate::bootstrap::state::AppState;
use crate::modules::payments::interfaces::http::handlers;
use crate::modules::store_tokens::interfaces::http::client_auth::store_client_auth_middleware;

pub fn client_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/payments/qris", post(handlers::create_qris_payment))
        .route("/payments/:paymentId", get(handlers::get_payment))
        .route(
            "/payments/:paymentId/status",
            get(handlers::get_payment_status),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            store_client_auth_middleware,
        ))
        .with_state(state)
}

pub fn dashboard_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", get(handlers::list_dashboard_payments))
        .route("/distribution", get(handlers::get_dashboard_payment_distribution))
        .route("/:paymentId", get(handlers::get_dashboard_payment))
        .with_state(state)
}

pub fn webhook_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/provider", post(handlers::provider_webhook))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    };

    use async_trait::async_trait;
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
        Json,
    };
    use bb8::Pool;
    use bb8_redis::{redis::AsyncCommands, RedisConnectionManager};
    use chrono::Utc;
    use reqwest::Url;
    use serde_json::{json, Value};
    use sqlx::postgres::PgPoolOptions;
    use tokio::net::TcpListener;
    use tower::ServiceExt;
    use uuid::Uuid;

    use super::*;
    use crate::bootstrap::config::Config;
    use crate::infrastructure::provider::config::QrisOtomatisConfig;
    use crate::infrastructure::provider::qris_otomatis::QrisOtomatisProvider;
    use crate::infrastructure::security::argon2::hash_secret;
    use crate::modules::balances::application::service::StoreBalanceService;
    use crate::modules::balances::domain::entity::{
        BalanceSummaryDelta, NewStoreBalanceLedgerEntry, StoreBalanceLedgerEntry,
        StoreBalanceSnapshot, StoreBalanceSummary,
    };
    use crate::modules::balances::domain::repository::StoreBalanceRepository;
    use crate::modules::auth::application::service::AuthService;
    use crate::modules::auth::domain::repository::AuthRepository;
    use crate::modules::auth::domain::session::Session;
    use crate::modules::auth::domain::user::AuthUser;
    use crate::modules::payments::application::idempotency::PaymentIdempotencyService;
    use crate::modules::payments::application::provider::{
        CheckDisbursementStatusRequest, CheckPaymentStatusRequest, CheckedDisbursementStatus,
        CheckedPaymentStatus, GenerateQrisRequest, GeneratedQris, GetBalanceRequest,
        InquiryBankRequest, InquiryBankResult, PaymentProviderGateway, ProviderBalanceSnapshot,
        TransferRequest, TransferResult,
    };
    use crate::modules::payments::application::service::PaymentService;
    use crate::modules::payments::domain::entity::{
        DashboardPaymentDetail, DashboardPaymentSummary, NewPaymentIdempotencyRecord,
        NewPaymentRecord, NewProviderWebhookEventRecord, Payment, PaymentIdempotencyRecord,
        PaymentIdempotencyStatus, PaymentPendingUpdate, PaymentStatus,
        StoreProviderProfile, CLIENT_PAYMENT_CREATE_RATE_LIMIT,
    };
    use crate::modules::notifications::application::service::NotificationService;
    use crate::modules::realtime::application::service::RealtimeService;
    use crate::modules::settlements::application::service::SettlementService;
    use crate::modules::settlements::infrastructure::repository::SqlxSettlementRepository;
    use crate::modules::store_banks::application::service::StoreBankService;
    use crate::modules::store_banks::infrastructure::repository::SqlxStoreBankRepository;
    use crate::modules::payments::domain::repository::{
        PaymentIdempotencyRepository, PaymentRepository,
    };
    use crate::modules::store_tokens::application::service::{
        CreateStoreApiTokenRequest, StoreTokenService,
    };
    use crate::modules::store_tokens::domain::entity::{
        is_store_api_token_expired, NewStoreApiTokenRecord, StoreApiTokenRecord,
    };
    use crate::modules::store_tokens::domain::repository::StoreTokenRepository;
    use crate::modules::stores::application::service::StoreService;
    use crate::modules::stores::domain::entity::{
        Store, StoreMember, StoreMemberDetail, StoreStatus, StoreSummary,
    };
    use crate::modules::stores::domain::repository::StoreRepository;
    use crate::modules::support::application::service::SupportService;
    use crate::modules::support::infrastructure::repository::SupportRepository;
    use crate::modules::users::application::service::UserService;
    use crate::modules::users::domain::entity::{User, UserStatus};
    use crate::modules::users::domain::repository::UserRepository;
    use crate::shared::audit::{AuditLogEntry, AuditLogRepository};
    use crate::shared::auth::{AuthenticatedUser, PlatformRole, StoreRole};
    use crate::shared::error::AppError;

    #[derive(Default)]
    struct MockAuditRepository;

    #[async_trait]
    impl AuditLogRepository for MockAuditRepository {
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
    struct MockUserRepository;

    #[async_trait]
    impl UserRepository for MockUserRepository {
        async fn find_by_id(&self, _id: Uuid) -> anyhow::Result<Option<User>> {
            Ok(None)
        }

        async fn find_by_email(&self, _email: &str) -> anyhow::Result<Option<User>> {
            Ok(None)
        }

        async fn list_memberships(
            &self,
            _user_id: Uuid,
        ) -> anyhow::Result<HashMap<Uuid, StoreRole>> {
            Ok(HashMap::new())
        }

        async fn user_exists_in_scope(
            &self,
            _actor_user_id: Uuid,
            _target_user_id: Uuid,
        ) -> anyhow::Result<bool> {
            Ok(false)
        }

        async fn list_users(
            &self,
            _limit: i64,
            _offset: i64,
            _role_filter: Option<PlatformRole>,
            _search: Option<&str>,
            _store_scope: Option<Uuid>,
        ) -> anyhow::Result<Vec<User>> {
            Ok(vec![])
        }

        async fn count_users(
            &self,
            _role_filter: Option<PlatformRole>,
            _search: Option<&str>,
            _store_scope: Option<Uuid>,
        ) -> anyhow::Result<i64> {
            Ok(0)
        }

        async fn create(&self, user: User) -> anyhow::Result<User> {
            Ok(user)
        }

        async fn update(&self, user: User) -> anyhow::Result<User> {
            Ok(user)
        }

        async fn update_status(&self, _id: Uuid, _status: UserStatus) -> anyhow::Result<()> {
            Ok(())
        }
    }

    struct NoopStoreRepository;

    #[async_trait]
    impl StoreRepository for NoopStoreRepository {
        async fn list_stores(
            &self,
            _limit: i64,
            _offset: i64,
            _search: Option<&str>,
            _status: Option<StoreStatus>,
            _user_scope: Option<Uuid>,
        ) -> anyhow::Result<Vec<StoreSummary>> {
            Ok(vec![])
        }

        async fn count_stores(
            &self,
            _search: Option<&str>,
            _status: Option<StoreStatus>,
            _user_scope: Option<Uuid>,
        ) -> anyhow::Result<i64> {
            Ok(0)
        }

        async fn find_store_by_id(&self, _id: Uuid) -> anyhow::Result<Option<Store>> {
            Ok(None)
        }

        async fn find_store_summary_by_id(
            &self,
            _id: Uuid,
        ) -> anyhow::Result<Option<StoreSummary>> {
            Ok(None)
        }

        async fn find_store_by_slug(&self, _slug: &str) -> anyhow::Result<Option<Store>> {
            Ok(None)
        }

        async fn create_store(
            &self,
            store: Store,
            _owner_member: StoreMember,
            _creator_member: Option<StoreMember>,
        ) -> anyhow::Result<Store> {
            Ok(store)
        }

        async fn update_store(&self, store: Store) -> anyhow::Result<Store> {
            Ok(store)
        }

        async fn list_members(&self, _store_id: Uuid) -> anyhow::Result<Vec<StoreMemberDetail>> {
            Ok(vec![])
        }

        async fn find_member_by_id(
            &self,
            _store_id: Uuid,
            _member_id: Uuid,
        ) -> anyhow::Result<Option<StoreMember>> {
            Ok(None)
        }

        async fn find_member_by_user_id(
            &self,
            _store_id: Uuid,
            _user_id: Uuid,
        ) -> anyhow::Result<Option<StoreMember>> {
            Ok(None)
        }

        async fn upsert_member(&self, _member: StoreMember) -> anyhow::Result<StoreMemberDetail> {
            unreachable!("unused in payment route tests")
        }

        async fn update_member(&self, _member: StoreMember) -> anyhow::Result<StoreMemberDetail> {
            unreachable!("unused in payment route tests")
        }

        async fn deactivate_member(&self, _store_id: Uuid, _member_id: Uuid) -> anyhow::Result<()> {
            Ok(())
        }
    }

    #[derive(Default)]
    struct NoopStoreBalanceRepository;

    #[async_trait]
    impl StoreBalanceRepository for NoopStoreBalanceRepository {
        async fn fetch_store_balance_snapshot(
            &self,
            _store_id: Uuid,
            _user_scope: Option<Uuid>,
            _global_access: bool,
        ) -> anyhow::Result<Option<StoreBalanceSnapshot>> {
            Ok(None)
        }

        async fn fetch_store_balance_summary(
            &self,
            _store_id: Uuid,
        ) -> anyhow::Result<Option<StoreBalanceSummary>> {
            Ok(None)
        }

        async fn apply_summary_delta(
            &self,
            _store_id: Uuid,
            _delta: BalanceSummaryDelta,
            _updated_at: chrono::DateTime<chrono::Utc>,
        ) -> anyhow::Result<StoreBalanceSummary> {
            Err(anyhow::anyhow!("balance summaries are not used in payment route tests"))
        }

        async fn insert_ledger_entry(
            &self,
            _entry: NewStoreBalanceLedgerEntry,
        ) -> anyhow::Result<StoreBalanceLedgerEntry> {
            Err(anyhow::anyhow!("balance ledger is not used in payment route tests"))
        }
    }

    #[derive(Default)]
    struct MockStoreTokenRepository {
        stores: Mutex<HashSet<Uuid>>,
        tokens: Mutex<Vec<StoreApiTokenRecord>>,
    }

    impl MockStoreTokenRepository {
        fn add_store(&self, store_id: Uuid) {
            self.stores.lock().unwrap().insert(store_id);
        }

        fn insert_existing_token(&self, token: StoreApiTokenRecord) {
            self.tokens.lock().unwrap().push(token);
        }
    }

    #[async_trait]
    impl StoreTokenRepository for MockStoreTokenRepository {
        async fn store_exists(&self, store_id: Uuid) -> anyhow::Result<bool> {
            Ok(self.stores.lock().unwrap().contains(&store_id))
        }

        async fn list_active_tokens(
            &self,
            store_id: Uuid,
        ) -> anyhow::Result<Vec<StoreApiTokenRecord>> {
            let now = Utc::now();
            Ok(self
                .tokens
                .lock()
                .unwrap()
                .iter()
                .filter(|token| {
                    token.store_id == store_id
                        && token.revoked_at.is_none()
                        && !is_store_api_token_expired(token.expires_at, now)
                })
                .cloned()
                .collect())
        }

        async fn insert_token(
            &self,
            token: NewStoreApiTokenRecord,
        ) -> anyhow::Result<StoreApiTokenRecord> {
            let record = StoreApiTokenRecord {
                id: token.id,
                store_id: token.store_id,
                name: token.name,
                token_prefix: token.token_prefix,
                token_hash: token.token_hash,
                last_used_at: None,
                expires_at: token.expires_at,
                revoked_at: None,
                created_by: token.created_by,
                created_at: token.created_at,
            };
            self.tokens.lock().unwrap().push(record.clone());
            Ok(record)
        }

        async fn find_token_by_lookup_prefix(
            &self,
            token_prefix: &str,
        ) -> anyhow::Result<Option<StoreApiTokenRecord>> {
            Ok(self
                .tokens
                .lock()
                .unwrap()
                .iter()
                .find(|token| token.token_prefix == token_prefix)
                .cloned())
        }

        async fn find_token_by_id(
            &self,
            store_id: Uuid,
            token_id: Uuid,
        ) -> anyhow::Result<Option<StoreApiTokenRecord>> {
            Ok(self
                .tokens
                .lock()
                .unwrap()
                .iter()
                .find(|token| token.store_id == store_id && token.id == token_id)
                .cloned())
        }

        async fn mark_token_revoked(
            &self,
            store_id: Uuid,
            token_id: Uuid,
            revoked_at: chrono::DateTime<Utc>,
        ) -> anyhow::Result<bool> {
            let mut tokens = self.tokens.lock().unwrap();
            let Some(token) = tokens
                .iter_mut()
                .find(|token| token.store_id == store_id && token.id == token_id)
            else {
                return Ok(false);
            };

            if token.revoked_at.is_some() {
                return Ok(false);
            }

            token.revoked_at = Some(revoked_at);
            Ok(true)
        }

        async fn touch_last_used_at(
            &self,
            token_id: Uuid,
            last_used_at: chrono::DateTime<Utc>,
        ) -> anyhow::Result<()> {
            if let Some(token) = self
                .tokens
                .lock()
                .unwrap()
                .iter_mut()
                .find(|token| token.id == token_id)
            {
                token.last_used_at = Some(last_used_at);
            }

            Ok(())
        }
    }

    #[derive(Default)]
    struct MockPaymentRepository {
        store_profiles: Mutex<HashMap<Uuid, StoreProviderProfile>>,
        payments: Mutex<HashMap<Uuid, Payment>>,
        idempotency: Mutex<HashMap<(Uuid, String), PaymentIdempotencyRecord>>,
    }

    impl MockPaymentRepository {
        fn add_store_profile(&self, store_id: Uuid, provider_username: &str) {
            self.store_profiles.lock().unwrap().insert(
                store_id,
                StoreProviderProfile {
                    store_id,
                    provider_username: provider_username.to_string(),
                },
            );
        }
    }

    #[async_trait]
    impl PaymentRepository for MockPaymentRepository {
        async fn find_store_provider_profile(
            &self,
            store_id: Uuid,
        ) -> anyhow::Result<Option<StoreProviderProfile>> {
            Ok(self.store_profiles.lock().unwrap().get(&store_id).cloned())
        }

        async fn insert_payment(&self, payment: NewPaymentRecord) -> anyhow::Result<Payment> {
            let mut payments = self.payments.lock().unwrap();
            let record = Payment {
                id: payment.id,
                store_id: payment.store_id,
                created_by_user_id: payment.created_by_user_id,
                provider_name: payment.provider_name,
                provider_terminal_id: Some(payment.provider_terminal_id),
                provider_trx_id: None,
                provider_rrn: None,
                merchant_order_id: payment.merchant_order_id,
                custom_ref: payment.custom_ref,
                gross_amount: payment.gross_amount,
                platform_tx_fee_bps: payment.platform_tx_fee_bps,
                platform_tx_fee_amount: payment.platform_tx_fee_amount,
                store_pending_credit_amount: payment.store_pending_credit_amount,
                status: payment.status,
                qris_payload: None,
                expired_at: payment.expired_at,
                provider_created_at: None,
                provider_finished_at: None,
                finalized_at: None,
                created_at: payment.created_at,
                updated_at: payment.updated_at,
            };
            payments.insert(record.id, record.clone());
            Ok(record)
        }

        async fn mark_payment_pending(
            &self,
            update: PaymentPendingUpdate,
        ) -> anyhow::Result<Payment> {
            let mut payments = self.payments.lock().unwrap();
            let payment = payments
                .get_mut(&update.payment_id)
                .ok_or_else(|| anyhow::anyhow!("payment not found"))?;
            payment.provider_trx_id = Some(update.provider_trx_id);
            payment.qris_payload = Some(update.qris_payload);
            payment.provider_created_at = update.provider_created_at;
            payment.status = PaymentStatus::Pending;
            payment.updated_at = update.updated_at;
            Ok(payment.clone())
        }

        async fn find_payment_by_id_for_store(
            &self,
            store_id: Uuid,
            payment_id: Uuid,
        ) -> anyhow::Result<Option<Payment>> {
            Ok(self
                .payments
                .lock()
                .unwrap()
                .get(&payment_id)
                .filter(|payment| payment.store_id == store_id)
                .cloned())
        }

        async fn list_dashboard_payments(
            &self,
            _limit: i64,
            _offset: i64,
            _search: Option<&str>,
            _status: Option<&str>,
            _user_scope: Option<Uuid>,
            _global_access: bool,
        ) -> anyhow::Result<Vec<DashboardPaymentSummary>> {
            Ok(vec![])
        }

        async fn count_dashboard_payments(
            &self,
            _search: Option<&str>,
            _status: Option<&str>,
            _user_scope: Option<Uuid>,
            _global_access: bool,
        ) -> anyhow::Result<i64> {
            Ok(0)
        }

        async fn find_dashboard_payment_by_id(
            &self,
            _payment_id: Uuid,
            _user_scope: Option<Uuid>,
            _global_access: bool,
        ) -> anyhow::Result<Option<DashboardPaymentDetail>> {
            Ok(None)
        }

        async fn count_dashboard_payment_distribution(
            &self,
            _user_scope: Option<Uuid>,
            _global_access: bool,
        ) -> anyhow::Result<crate::modules::payments::domain::entity::DashboardPaymentDistribution>
        {
            Ok(crate::modules::payments::domain::entity::DashboardPaymentDistribution {
                success: 0,
                failed: 0,
                expired: 0,
            })
        }

        async fn find_payment_by_provider_trx_id(
            &self,
            _provider_name: &str,
            _provider_trx_id: &str,
        ) -> anyhow::Result<Option<crate::modules::payments::domain::entity::PaymentWebhookTarget>>
        {
            Ok(None)
        }

        async fn insert_provider_webhook_event(
            &self,
            _event: NewProviderWebhookEventRecord,
        ) -> anyhow::Result<crate::modules::payments::domain::entity::ProviderWebhookEvent> {
            unreachable!("unused in client payment route tests")
        }

        async fn mark_provider_webhook_event_result(
            &self,
            _event_id: Uuid,
            _is_verified: bool,
            _verification_reason: Option<&str>,
            _is_processed: bool,
            _processing_result: Option<&str>,
            _processed_at: Option<chrono::DateTime<Utc>>,
        ) -> anyhow::Result<crate::modules::payments::domain::entity::ProviderWebhookEvent> {
            unreachable!("unused in client payment route tests")
        }

        async fn finalize_payment_from_webhook(
            &self,
            _command: crate::modules::payments::domain::entity::PaymentWebhookFinalizeCommand,
        ) -> anyhow::Result<crate::modules::payments::domain::entity::PaymentWebhookFinalizeOutcome>
        {
            unreachable!("unused in client payment route tests")
        }
    }

    #[async_trait]
    impl PaymentIdempotencyRepository for MockPaymentRepository {
        async fn find_by_key(
            &self,
            store_id: Uuid,
            idempotency_key: &str,
        ) -> anyhow::Result<Option<PaymentIdempotencyRecord>> {
            Ok(self
                .idempotency
                .lock()
                .unwrap()
                .get(&(store_id, idempotency_key.to_string()))
                .cloned())
        }

        async fn insert_pending(
            &self,
            record: NewPaymentIdempotencyRecord,
        ) -> anyhow::Result<PaymentIdempotencyRecord> {
            let key = (record.store_id, record.idempotency_key.clone());
            let mut entries = self.idempotency.lock().unwrap();
            if entries.contains_key(&key) {
                return Err(sqlx::Error::Protocol("duplicate idempotency".into()).into());
            }

            let stored = PaymentIdempotencyRecord {
                id: record.id,
                store_id: record.store_id,
                idempotency_key: record.idempotency_key,
                request_hash: record.request_hash,
                status: record.status,
                response_status_code: None,
                response_body_json: None,
                payment_id: None,
                created_at: record.created_at,
                completed_at: None,
                updated_at: record.updated_at,
            };
            entries.insert(key, stored.clone());
            Ok(stored)
        }

        async fn complete(
            &self,
            store_id: Uuid,
            idempotency_key: &str,
            response_status_code: i32,
            response_body_json: Value,
            payment_id: Option<Uuid>,
            completed_at: chrono::DateTime<Utc>,
        ) -> anyhow::Result<PaymentIdempotencyRecord> {
            let key = (store_id, idempotency_key.to_string());
            let mut entries = self.idempotency.lock().unwrap();
            let record = entries
                .get_mut(&key)
                .ok_or_else(|| anyhow::anyhow!("idempotency key not found"))?;
            record.status = PaymentIdempotencyStatus::Completed;
            record.response_status_code = Some(response_status_code);
            record.response_body_json = Some(response_body_json);
            record.payment_id = payment_id;
            record.completed_at = Some(completed_at);
            record.updated_at = completed_at;
            Ok(record.clone())
        }
    }

    #[derive(Clone, Default)]
    struct MockProvider {
        calls: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl PaymentProviderGateway for MockProvider {
        async fn generate_qris(
            &self,
            _request: GenerateQrisRequest,
        ) -> Result<GeneratedQris, AppError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(GeneratedQris {
                provider_trx_id: format!("trx-{}", self.calls.load(Ordering::SeqCst)),
                qris_payload: "mock-qris-payload".into(),
            })
        }

        async fn check_payment_status(
            &self,
            _request: CheckPaymentStatusRequest,
        ) -> Result<CheckedPaymentStatus, AppError> {
            unreachable!("not used in payment route tests")
        }

        async fn inquiry_bank(
            &self,
            _request: InquiryBankRequest,
        ) -> Result<InquiryBankResult, AppError> {
            unreachable!("not used in payment route tests")
        }

        async fn transfer(&self, _request: TransferRequest) -> Result<TransferResult, AppError> {
            unreachable!("not used in payment route tests")
        }

        async fn check_disbursement_status(
            &self,
            _request: CheckDisbursementStatusRequest,
        ) -> Result<CheckedDisbursementStatus, AppError> {
            unreachable!("not used in payment route tests")
        }

        async fn get_balance(
            &self,
            _request: GetBalanceRequest,
        ) -> Result<ProviderBalanceSnapshot, AppError> {
            unreachable!("not used in payment route tests")
        }
    }

    fn owner_actor(store_id: Uuid) -> AuthenticatedUser {
        AuthenticatedUser {
            user_id: Uuid::new_v4(),
            platform_role: PlatformRole::User,
            memberships: HashMap::from([(store_id, StoreRole::Owner)]),
        }
    }

    async fn clear_rate_limit(redis: &Pool<RedisConnectionManager>, store_id: Uuid) {
        let key = format!("limiter:client_payment_create:store:{store_id}");
        let mut conn = redis.get().await.unwrap();
        let _: () = conn.del(key).await.unwrap();
    }

    fn test_router(
        payment_repository: Arc<MockPaymentRepository>,
        token_repository: Arc<MockStoreTokenRepository>,
        provider: Arc<MockProvider>,
    ) -> (Router, Arc<StoreTokenService>, Pool<RedisConnectionManager>) {
        test_router_with_provider(payment_repository, token_repository, provider)
    }

    fn test_router_with_provider(
        payment_repository: Arc<MockPaymentRepository>,
        token_repository: Arc<MockStoreTokenRepository>,
        provider: Arc<dyn PaymentProviderGateway>,
    ) -> (Router, Arc<StoreTokenService>, Pool<RedisConnectionManager>) {
        let db = PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost/justqiu_test")
            .unwrap();
        let redis_manager = RedisConnectionManager::new("redis://127.0.0.1/").unwrap();
        let redis = Pool::builder().build_unchecked(redis_manager);
        let captcha = Arc::new(crate::infrastructure::security::captcha::NoOpCaptchaVerifier);
        let auth_service = Arc::new(AuthService::new(
            Arc::new(MockAuthRepository),
            captcha.clone(),
            redis.clone(),
        ));
        let user_service = Arc::new(UserService::new(
            Arc::new(MockUserRepository),
            Arc::new(MockAuditRepository),
        ));
        let support_service = Arc::new(SupportService::new(
            SupportRepository::new(db.clone()),
            captcha,
        ));
        let store_service = Arc::new(StoreService::new(
            Arc::new(NoopStoreRepository),
            Arc::new(
                crate::modules::users::infrastructure::repository::SqlxUserRepository::new(
                    db.clone(),
                ),
            ),
            Arc::new(MockAuditRepository),
        ));
        let store_token_service = Arc::new(StoreTokenService::new(
            token_repository,
            Arc::new(MockAuditRepository),
        ));
        let balance_service = Arc::new(StoreBalanceService::new(Arc::new(
            NoopStoreBalanceRepository,
        )));
        let payment_service = Arc::new(PaymentService::new(
            payment_repository.clone(),
            provider.clone(),
        ));
        let payment_idempotency_service =
            Arc::new(PaymentIdempotencyService::new(payment_repository));
        let notification_service = Arc::new(NotificationService::new(Arc::new(
            crate::modules::notifications::infrastructure::repository::SqlxNotificationRepository::new(
                db.clone(),
            ),
        )));
        let realtime_service = Arc::new(RealtimeService::new(64));
        let settlement_service = Arc::new(SettlementService::new(Arc::new(
            SqlxSettlementRepository::new(db.clone()),
        )));
        let store_bank_service = Arc::new(StoreBankService::new(
            Arc::new(SqlxStoreBankRepository::new(
                db.clone(),
                "bank-test-key".into(),
            )),
            Arc::new(
                crate::modules::store_banks::infrastructure::cache::RedisStoreBankInquiryCache::new(
                    redis.clone(),
                ),
            ),
            provider.clone(),
            Arc::new(MockAuditRepository),
        ));

        let state = AppState {
            config: Config {
                port: 0,
                database_url: "postgres://postgres:postgres@localhost/justqiu_test".into(),
                redis_url: "redis://127.0.0.1/".into(),
                log_level: "info".into(),
                store_bank_account_encryption_key: "bank-test-key".into(),
                external_api_url: "http://127.0.0.1".into(),
                external_api_uuid: "uuid".into(),
                external_api_client: "client".into(),
                external_api_secret: "secret".into(),
                external_api_timeout_seconds: 5,
            },
            db,
            redis: redis.clone(),
            auth_service,
            balance_service,
            notification_service,
            payment_idempotency_service,
            payment_service,
            realtime_service,
            settlement_service,
            store_bank_service,
            store_service,
            store_token_service: store_token_service.clone(),
            support_service,
            user_service,
        };

        let app = client_routes(state.clone()).with_state(state);

        (app, store_token_service, redis)
    }

    async fn spawn_real_provider_server() -> String {
        async fn handle_generate(Json(payload): Json<Value>) -> Json<Value> {
            Json(json!({
                "status": true,
                "data": format!("qris-for-{}", payload["username"].as_str().unwrap_or("unknown")),
                "trx_id": "real-adapter-trx-1"
            }))
        }

        let app = Router::new().route("/api/generate", post(handle_generate));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        format!("http://{addr}/")
    }

    async fn create_store_token(
        store_token_service: Arc<StoreTokenService>,
        store_id: Uuid,
    ) -> String {
        store_token_service
            .create_token(
                store_id,
                CreateStoreApiTokenRequest {
                    name: "Client Token".into(),
                },
                &owner_actor(store_id),
            )
            .await
            .unwrap()
            .plaintext_token
    }

    async fn create_payment_request(
        app: &Router,
        bearer_token: &str,
        idempotency_key: &str,
        merchant_order_id: &str,
    ) -> axum::response::Response {
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/payments/qris")
                    .header("Authorization", format!("Bearer {bearer_token}"))
                    .header("Idempotency-Key", idempotency_key)
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "amount": 10000,
                            "expire_seconds": 300,
                            "custom_ref": "customer-1",
                            "merchant_order_id": merchant_order_id
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn create_payment_route_success_and_idempotency_cache_are_identical() {
        let store_id = Uuid::new_v4();
        let payment_repository = Arc::new(MockPaymentRepository::default());
        payment_repository.add_store_profile(store_id, "provider-store");
        let token_repository = Arc::new(MockStoreTokenRepository::default());
        token_repository.add_store(store_id);
        let provider = Arc::new(MockProvider::default());
        let (app, store_token_service, redis) = test_router(
            payment_repository.clone(),
            token_repository,
            provider.clone(),
        );
        clear_rate_limit(&redis, store_id).await;
        let bearer_token = create_store_token(store_token_service, store_id).await;

        let first = create_payment_request(&app, &bearer_token, "idem-1", "order-1").await;
        assert_eq!(first.status(), StatusCode::CREATED);
        let first_body = to_bytes(first.into_body(), usize::MAX).await.unwrap();

        let second = create_payment_request(&app, &bearer_token, "idem-1", "order-1").await;
        assert_eq!(second.status(), StatusCode::CREATED);
        let second_body = to_bytes(second.into_body(), usize::MAX).await.unwrap();

        assert_eq!(first_body, second_body);
        assert_eq!(provider.calls.load(Ordering::SeqCst), 1);

        let payload: Value = serde_json::from_slice(&second_body).unwrap();
        assert_eq!(payload["payment"]["status"], json!("pending"));
        assert_eq!(
            payload["payment"]["qris_payload"],
            json!("mock-qris-payload")
        );
    }

    #[tokio::test]
    async fn create_payment_route_still_works_with_real_provider_adapter() {
        let store_id = Uuid::new_v4();
        let payment_repository = Arc::new(MockPaymentRepository::default());
        payment_repository.add_store_profile(store_id, "provider-store");
        let token_repository = Arc::new(MockStoreTokenRepository::default());
        token_repository.add_store(store_id);
        let provider_base_url = spawn_real_provider_server().await;
        let provider = Arc::new(
            QrisOtomatisProvider::new(QrisOtomatisConfig {
                base_url: Url::parse(&provider_base_url).unwrap(),
                merchant_uuid: "merchant-uuid".into(),
                client_name: "client-name".into(),
                client_key: "client-key".into(),
                timeout: std::time::Duration::from_secs(5),
            })
            .unwrap(),
        );
        let (app, store_token_service, redis) =
            test_router_with_provider(payment_repository, token_repository, provider);
        clear_rate_limit(&redis, store_id).await;
        let bearer_token = create_store_token(store_token_service, store_id).await;

        let response = create_payment_request(&app, &bearer_token, "idem-real", "order-real").await;
        assert_eq!(response.status(), StatusCode::CREATED);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: Value = serde_json::from_slice(&body).unwrap();

        println!(
            "{}",
            json!({
                "payment_id": payload["payment"]["id"],
                "provider_trx_id": payload["payment"]["provider_trx_id"],
                "qris_payload": payload["payment"]["qris_payload"],
                "status": payload["payment"]["status"],
            })
        );

        assert_eq!(payload["payment"]["status"], json!("pending"));
        assert_eq!(
            payload["payment"]["provider_trx_id"],
            json!("real-adapter-trx-1")
        );
        assert_eq!(
            payload["payment"]["qris_payload"],
            json!("qris-for-provider-store")
        );
    }

    #[tokio::test]
    async fn create_payment_route_rejects_idempotency_key_mismatch() {
        let store_id = Uuid::new_v4();
        let payment_repository = Arc::new(MockPaymentRepository::default());
        payment_repository.add_store_profile(store_id, "provider-store");
        let token_repository = Arc::new(MockStoreTokenRepository::default());
        token_repository.add_store(store_id);
        let provider = Arc::new(MockProvider::default());
        let (app, store_token_service, redis) =
            test_router(payment_repository, token_repository, provider);
        clear_rate_limit(&redis, store_id).await;
        let bearer_token = create_store_token(store_token_service, store_id).await;

        let first = create_payment_request(&app, &bearer_token, "idem-2", "order-2").await;
        assert_eq!(first.status(), StatusCode::CREATED);

        let mismatch = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/payments/qris")
                    .header("Authorization", format!("Bearer {bearer_token}"))
                    .header("Idempotency-Key", "idem-2")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "amount": 12000,
                            "expire_seconds": 300,
                            "merchant_order_id": "order-3"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(mismatch.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn create_payment_route_requires_idempotency_key_header() {
        let store_id = Uuid::new_v4();
        let payment_repository = Arc::new(MockPaymentRepository::default());
        payment_repository.add_store_profile(store_id, "provider-store");
        let token_repository = Arc::new(MockStoreTokenRepository::default());
        token_repository.add_store(store_id);
        let provider = Arc::new(MockProvider::default());
        let (app, store_token_service, redis) =
            test_router(payment_repository, token_repository, provider);
        clear_rate_limit(&redis, store_id).await;
        let bearer_token = create_store_token(store_token_service, store_id).await;

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/payments/qris")
                    .header("Authorization", format!("Bearer {bearer_token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "amount": 10000,
                            "expire_seconds": 300
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn create_payment_route_requires_valid_token_and_rejects_revoked_tokens() {
        let store_id = Uuid::new_v4();
        let payment_repository = Arc::new(MockPaymentRepository::default());
        payment_repository.add_store_profile(store_id, "provider-store");
        let token_repository = Arc::new(MockStoreTokenRepository::default());
        token_repository.add_store(store_id);
        let provider = Arc::new(MockProvider::default());
        let (app, store_token_service, redis) =
            test_router(payment_repository, token_repository, provider);
        clear_rate_limit(&redis, store_id).await;

        let missing = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/payments/qris")
                    .header("Idempotency-Key", "idem-missing")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "amount": 10000,
                            "expire_seconds": 300
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);

        let invalid = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/payments/qris")
                    .header("Authorization", "Bearer invalid")
                    .header("Idempotency-Key", "idem-invalid")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "amount": 10000,
                            "expire_seconds": 300
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(invalid.status(), StatusCode::UNAUTHORIZED);

        let bearer_token = create_store_token(store_token_service.clone(), store_id).await;
        let listed = store_token_service
            .list_tokens(store_id, &owner_actor(store_id))
            .await
            .unwrap();
        store_token_service
            .revoke_token(store_id, listed[0].id, &owner_actor(store_id))
            .await
            .unwrap();

        let revoked =
            create_payment_request(&app, &bearer_token, "idem-revoked", "order-revoked").await;
        assert_eq!(revoked.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn create_payment_route_rejects_expired_tokens() {
        let store_id = Uuid::new_v4();
        let payment_repository = Arc::new(MockPaymentRepository::default());
        payment_repository.add_store_profile(store_id, "provider-store");
        let token_repository = Arc::new(MockStoreTokenRepository::default());
        token_repository.add_store(store_id);
        let provider = Arc::new(MockProvider::default());
        let (app, _store_token_service, redis) =
            test_router(payment_repository, token_repository.clone(), provider);
        clear_rate_limit(&redis, store_id).await;

        let expired_plaintext = "jq_sk_aaaaaaaaaaaa.secretexpired";
        token_repository.insert_existing_token(StoreApiTokenRecord {
            id: Uuid::new_v4(),
            store_id,
            name: "Expired".into(),
            token_prefix: "jq_sk_aaaaaaaaaaaa".into(),
            token_hash: hash_secret(expired_plaintext).unwrap(),
            last_used_at: None,
            expires_at: Some(Utc::now() - chrono::Duration::minutes(1)),
            revoked_at: None,
            created_by: Uuid::new_v4(),
            created_at: Utc::now(),
        });

        let response =
            create_payment_request(&app, expired_plaintext, "idem-expired", "order-expired").await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn create_payment_rate_limit_is_enforced() {
        let store_id = Uuid::new_v4();
        let payment_repository = Arc::new(MockPaymentRepository::default());
        payment_repository.add_store_profile(store_id, "provider-store");
        let token_repository = Arc::new(MockStoreTokenRepository::default());
        token_repository.add_store(store_id);
        let provider = Arc::new(MockProvider::default());
        let (app, store_token_service, redis) =
            test_router(payment_repository, token_repository, provider.clone());
        clear_rate_limit(&redis, store_id).await;
        let bearer_token = create_store_token(store_token_service, store_id).await;

        for index in 0..CLIENT_PAYMENT_CREATE_RATE_LIMIT {
            let response = create_payment_request(
                &app,
                &bearer_token,
                &format!("idem-rate-{index}"),
                &format!("order-rate-{index}"),
            )
            .await;
            assert_eq!(response.status(), StatusCode::CREATED);
        }

        let blocked =
            create_payment_request(&app, &bearer_token, "idem-rate-final", "order-rate-final")
                .await;
        assert_eq!(blocked.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(
            provider.calls.load(Ordering::SeqCst),
            CLIENT_PAYMENT_CREATE_RATE_LIMIT as usize
        );
    }

    #[tokio::test]
    async fn detail_and_status_routes_are_store_scoped() {
        let store_a = Uuid::new_v4();
        let store_b = Uuid::new_v4();
        let payment_repository = Arc::new(MockPaymentRepository::default());
        payment_repository.add_store_profile(store_a, "provider-a");
        payment_repository.add_store_profile(store_b, "provider-b");
        let token_repository = Arc::new(MockStoreTokenRepository::default());
        token_repository.add_store(store_a);
        token_repository.add_store(store_b);
        let provider = Arc::new(MockProvider::default());
        let (app, store_token_service, redis) =
            test_router(payment_repository, token_repository, provider);
        clear_rate_limit(&redis, store_a).await;
        clear_rate_limit(&redis, store_b).await;
        let token_a = create_store_token(store_token_service.clone(), store_a).await;
        let token_b = create_store_token(store_token_service, store_b).await;

        let created = create_payment_request(&app, &token_a, "idem-scope-a", "order-scope-a").await;
        assert_eq!(created.status(), StatusCode::CREATED);
        let created_body = to_bytes(created.into_body(), usize::MAX).await.unwrap();
        let created_payload: Value = serde_json::from_slice(&created_body).unwrap();
        let payment_id = created_payload["payment"]["id"].as_str().unwrap();

        let detail_allowed = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/payments/{payment_id}"))
                    .header("Authorization", format!("Bearer {token_a}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(detail_allowed.status(), StatusCode::OK);

        let detail_blocked = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/payments/{payment_id}"))
                    .header("Authorization", format!("Bearer {token_b}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(detail_blocked.status(), StatusCode::NOT_FOUND);

        let status_allowed = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/payments/{payment_id}/status"))
                    .header("Authorization", format!("Bearer {token_a}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(status_allowed.status(), StatusCode::OK);

        let status_blocked = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/payments/{payment_id}/status"))
                    .header("Authorization", format!("Bearer {token_b}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(status_blocked.status(), StatusCode::NOT_FOUND);
    }
}
