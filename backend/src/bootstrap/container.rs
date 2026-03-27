use super::config::Config;
use super::state::AppState;
use crate::infrastructure::db::init_db_pool;
use crate::infrastructure::provider::config::QrisOtomatisConfig;
use crate::infrastructure::provider::qris_otomatis::QrisOtomatisProvider;
use crate::infrastructure::redis::init_redis_pool;
use crate::modules::balances::application::service::StoreBalanceService;
use crate::modules::balances::infrastructure::repository::SqlxStoreBalanceRepository;
use crate::modules::notifications::application::service::NotificationService;
use crate::modules::payments::application::idempotency::PaymentIdempotencyService;
use crate::modules::payments::application::service::PaymentService;
use crate::modules::realtime::application::service::RealtimeService;
use crate::modules::settlements::application::service::SettlementService;
use crate::modules::settlements::infrastructure::repository::SqlxSettlementRepository;
use crate::shared::error::AppError;
use std::sync::Arc;
use tracing::info;

pub struct Container;

impl Container {
    pub async fn build() -> Result<Arc<AppState>, AppError> {
        info!("Building application container...");

        let config = Config::from_env()?;

        let db = init_db_pool(&config.database_url).await?;
        let redis = init_redis_pool(&config.redis_url).await?;

        let repo = Arc::new(
            crate::modules::auth::infrastructure::persistence::PostgresAuthRepository::new(
                db.clone(),
            ),
        );
        let captcha: Arc<dyn crate::infrastructure::security::captcha::CaptchaVerifier> =
            Arc::new(crate::infrastructure::security::captcha::NoOpCaptchaVerifier);
        let auth_service = Arc::new(
            crate::modules::auth::application::service::AuthService::new(
                repo,
                captcha.clone(),
                redis.clone(),
            ),
        );

        let support_repo =
            crate::modules::support::infrastructure::repository::SupportRepository::new(db.clone());
        let support_service = Arc::new(
            crate::modules::support::application::service::SupportService::new(
                support_repo,
                captcha.clone(),
            ),
        );

        let user_repo = Arc::new(
            crate::modules::users::infrastructure::repository::SqlxUserRepository::new(db.clone()),
        );
        let audit_repo = Arc::new(crate::infrastructure::audit::SqlxAuditLogRepository::new(
            db.clone(),
        ));
        let user_service = Arc::new(
            crate::modules::users::application::service::UserService::new(
                user_repo.clone(),
                audit_repo,
            ),
        );
        let balance_repository =
            Arc::new(SqlxStoreBalanceRepository::new(db.clone()));
        let balance_service = Arc::new(StoreBalanceService::new(balance_repository));
        let store_repo = Arc::new(
            crate::modules::stores::infrastructure::repository::SqlxStoreRepository::new(
                db.clone(),
            ),
        );
        let store_audit_repo = Arc::new(crate::infrastructure::audit::SqlxAuditLogRepository::new(
            db.clone(),
        ));
        let store_service = Arc::new(
            crate::modules::stores::application::service::StoreService::new(
                store_repo,
                user_repo,
                store_audit_repo,
            ),
        );
        let store_token_repo = Arc::new(
            crate::modules::store_tokens::infrastructure::repository::SqlxStoreTokenRepository::new(
                db.clone(),
            ),
        );
        let store_token_audit_repo = Arc::new(
            crate::infrastructure::audit::SqlxAuditLogRepository::new(db.clone()),
        );
        let store_token_service = Arc::new(
            crate::modules::store_tokens::application::service::StoreTokenService::new(
                store_token_repo,
                store_token_audit_repo,
            ),
        );
        let payment_repository = Arc::new(
            crate::modules::payments::infrastructure::repository::SqlxPaymentRepository::new(
                db.clone(),
            ),
        );
        let notification_repository = Arc::new(
            crate::modules::notifications::infrastructure::repository::SqlxNotificationRepository::new(
                db.clone(),
            ),
        );
        let provider_adapter = Arc::new(QrisOtomatisProvider::new(
            QrisOtomatisConfig::from_app_config(&config)?,
        )?);
        let payment_service = Arc::new(PaymentService::new(
            payment_repository.clone(),
            provider_adapter,
        ));
        let payment_idempotency_service =
            Arc::new(PaymentIdempotencyService::new(payment_repository));
        let notification_service = Arc::new(NotificationService::new(notification_repository));
        let realtime_service = Arc::new(RealtimeService::new(256));
        let settlement_repository =
            Arc::new(SqlxSettlementRepository::new(db.clone()));
        let settlement_service = Arc::new(SettlementService::new(settlement_repository));

        let state = Arc::new(AppState {
            config,
            db,
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
        });

        info!("Application container built successfully.");
        Ok(state)
    }
}
