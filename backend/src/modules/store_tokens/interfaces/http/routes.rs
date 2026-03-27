use axum::Router;

use crate::bootstrap::state::AppState;
use crate::modules::store_tokens::interfaces::http::handlers;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/:storeId/tokens",
            axum::routing::get(handlers::list_tokens).post(handlers::create_token),
        )
        .route(
            "/:storeId/tokens/:tokenId",
            axum::routing::delete(handlers::revoke_token),
        )
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
        middleware, Router,
    };
    use bb8::Pool;
    use bb8_redis::RedisConnectionManager;
    use chrono::Utc;
    use serde_json::Value;
    use sqlx::postgres::PgPoolOptions;
    use tower::ServiceExt;
    use uuid::Uuid;

    use super::*;
    use crate::bootstrap::config::Config;
    use crate::modules::auth::application::dto::{SessionContext, UserProfile};
    use crate::modules::auth::application::service::AuthService;
    use crate::modules::auth::domain::repository::AuthRepository;
    use crate::modules::auth::domain::session::Session;
    use crate::modules::auth::domain::user::AuthUser;
    use crate::modules::auth::interfaces::http::middlewares::csrf_middleware;
    use crate::modules::payments::application::idempotency::PaymentIdempotencyService;
    use crate::modules::payments::application::provider::{
        CheckDisbursementStatusRequest, CheckPaymentStatusRequest, CheckedDisbursementStatus,
        CheckedPaymentStatus, GenerateQrisRequest, GeneratedQris, GetBalanceRequest,
        InquiryBankRequest, InquiryBankResult, PaymentProviderGateway, ProviderBalanceSnapshot,
        TransferRequest, TransferResult,
    };
    use crate::modules::payments::application::service::PaymentService;
    use crate::modules::store_tokens::application::service::StoreTokenService;
    use crate::modules::store_tokens::domain::entity::{
        NewStoreApiTokenRecord, StoreApiTokenRecord,
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
    use crate::shared::auth::{PlatformRole, StoreRole};
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
    struct MockUserRepository {
        memberships: Mutex<HashMap<Uuid, HashMap<Uuid, StoreRole>>>,
    }

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
            user_id: Uuid,
        ) -> anyhow::Result<HashMap<Uuid, StoreRole>> {
            Ok(self
                .memberships
                .lock()
                .unwrap()
                .get(&user_id)
                .cloned()
                .unwrap_or_default())
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

    #[derive(Default)]
    struct MockStoreTokenRepository {
        stores: Mutex<HashSet<Uuid>>,
        tokens: Mutex<Vec<StoreApiTokenRecord>>,
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
            Ok(self
                .tokens
                .lock()
                .unwrap()
                .iter()
                .filter(|token| token.store_id == store_id && token.revoked_at.is_none())
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
            if let Some(token) = self
                .tokens
                .lock()
                .unwrap()
                .iter_mut()
                .find(|token| token.store_id == store_id && token.id == token_id)
            {
                if token.revoked_at.is_some() {
                    return Ok(false);
                }

                token.revoked_at = Some(revoked_at);
                return Ok(true);
            }

            Ok(false)
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

    struct NoopStoreRepository;

    struct NoopProvider;

    #[async_trait]
    impl PaymentProviderGateway for NoopProvider {
        async fn generate_qris(
            &self,
            _request: GenerateQrisRequest,
        ) -> Result<GeneratedQris, AppError> {
            unreachable!("payment service is unused in store token route tests")
        }

        async fn check_payment_status(
            &self,
            _request: CheckPaymentStatusRequest,
        ) -> Result<CheckedPaymentStatus, AppError> {
            unreachable!("payment service is unused in store token route tests")
        }

        async fn inquiry_bank(
            &self,
            _request: InquiryBankRequest,
        ) -> Result<InquiryBankResult, AppError> {
            unreachable!("payment service is unused in store token route tests")
        }

        async fn transfer(&self, _request: TransferRequest) -> Result<TransferResult, AppError> {
            unreachable!("payment service is unused in store token route tests")
        }

        async fn check_disbursement_status(
            &self,
            _request: CheckDisbursementStatusRequest,
        ) -> Result<CheckedDisbursementStatus, AppError> {
            unreachable!("payment service is unused in store token route tests")
        }

        async fn get_balance(
            &self,
            _request: GetBalanceRequest,
        ) -> Result<ProviderBalanceSnapshot, AppError> {
            unreachable!("payment service is unused in store token route tests")
        }
    }

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
            unreachable!("store service is unused in store token route tests")
        }

        async fn update_member(&self, _member: StoreMember) -> anyhow::Result<StoreMemberDetail> {
            unreachable!("store service is unused in store token route tests")
        }

        async fn deactivate_member(&self, _store_id: Uuid, _member_id: Uuid) -> anyhow::Result<()> {
            Ok(())
        }
    }

    fn test_router(owner_user_id: Uuid, store_id: Uuid) -> Router {
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

        let user_repository = Arc::new(MockUserRepository::default());
        user_repository
            .memberships
            .lock()
            .unwrap()
            .insert(owner_user_id, HashMap::from([(store_id, StoreRole::Owner)]));
        let user_service = Arc::new(UserService::new(
            user_repository,
            Arc::new(MockAuditRepository),
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
        let support_service = Arc::new(SupportService::new(
            SupportRepository::new(db.clone()),
            captcha,
        ));

        let token_repository = Arc::new(MockStoreTokenRepository::default());
        token_repository.stores.lock().unwrap().insert(store_id);
        let store_token_service = Arc::new(StoreTokenService::new(
            token_repository,
            Arc::new(MockAuditRepository),
        ));
        let payment_repository = Arc::new(
            crate::modules::payments::infrastructure::repository::SqlxPaymentRepository::new(
                db.clone(),
            ),
        );
        let payment_service = Arc::new(PaymentService::new(
            payment_repository.clone(),
            Arc::new(NoopProvider),
        ));
        let payment_idempotency_service =
            Arc::new(PaymentIdempotencyService::new(payment_repository));

        let state = AppState {
            config: Config {
                port: 0,
                database_url: "postgres://postgres:postgres@localhost/justqiu_test".into(),
                redis_url: "redis://127.0.0.1/".into(),
                log_level: "info".into(),
                external_api_url: "https://example.com".into(),
                external_api_uuid: "test-uuid".into(),
                external_api_client: "test-client".into(),
                external_api_secret: "test-secret".into(),
                external_api_timeout_seconds: 5,
            },
            db,
            redis,
            auth_service,
            payment_idempotency_service,
            payment_service,
            store_service,
            store_token_service,
            support_service,
            user_service,
        };

        routes()
            .layer(middleware::from_fn(csrf_middleware))
            .with_state(state)
    }

    fn session_context(user_id: Uuid, csrf_token: &str) -> SessionContext {
        SessionContext {
            session_id: Uuid::new_v4(),
            user: UserProfile {
                id: user_id,
                name: "Owner".into(),
                email: "owner@example.com".into(),
                role: "user".into(),
                status: "active".into(),
            },
            csrf_token: csrf_token.to_string(),
        }
    }

    #[tokio::test]
    async fn create_token_route_requires_valid_csrf_header() {
        let owner_user_id = Uuid::new_v4();
        let store_id = Uuid::new_v4();
        let app = test_router(owner_user_id, store_id);

        let mut missing_csrf_request = Request::builder()
            .method("POST")
            .uri(format!("/{store_id}/tokens"))
            .header("content-type", "application/json")
            .body(Body::from(r#"{"name":"Dashboard Token"}"#))
            .unwrap();
        missing_csrf_request
            .extensions_mut()
            .insert(session_context(owner_user_id, "csrf-123"));

        let missing_csrf_response = app.clone().oneshot(missing_csrf_request).await.unwrap();
        assert_eq!(missing_csrf_response.status(), StatusCode::FORBIDDEN);

        let mut valid_request = Request::builder()
            .method("POST")
            .uri(format!("/{store_id}/tokens"))
            .header("content-type", "application/json")
            .header("X-CSRF-Token", "csrf-123")
            .body(Body::from(r#"{"name":"Dashboard Token"}"#))
            .unwrap();
        valid_request
            .extensions_mut()
            .insert(session_context(owner_user_id, "csrf-123"));

        let valid_response = app.oneshot(valid_request).await.unwrap();
        assert_eq!(valid_response.status(), StatusCode::CREATED);

        let body = to_bytes(valid_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let payload: Value = serde_json::from_slice(&body).unwrap();
        assert!(payload["plaintext_token"]
            .as_str()
            .unwrap()
            .starts_with("jq_sk_"));
    }
}
