use std::sync::Arc;

use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Error as SqlxError;
use uuid::Uuid;

use crate::modules::payments::application::provider::{
    GenerateQrisRequest, PaymentProviderGateway,
};
use crate::modules::payments::domain::entity::{
    payment_to_detail, payment_to_status_view, ClientPaymentDetail, ClientPaymentStatusView,
    NewPaymentRecord, PaymentPendingUpdate, PaymentStatus, StoreProviderProfile,
    CLIENT_PAYMENT_MAX_EXPIRE_SECONDS, CLIENT_PAYMENT_MIN_EXPIRE_SECONDS, PAYMENT_PLATFORM_FEE_BPS,
    PAYMENT_PROVIDER_NAME,
};
use crate::modules::payments::domain::repository::PaymentRepository;
use crate::modules::store_tokens::domain::entity::StoreApiTokenAuthContext;
use crate::shared::error::AppError;
use crate::shared::money::payment_fee_breakdown;

#[derive(Debug, Clone, Deserialize)]
pub struct CreateClientPaymentRequest {
    pub amount: i64,
    pub expire_seconds: i64,
    pub custom_ref: Option<String>,
    pub merchant_order_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NormalizedCreateClientPaymentRequest {
    pub amount: i64,
    pub expire_seconds: i64,
    pub custom_ref: Option<String>,
    pub merchant_order_id: Option<String>,
}

impl CreateClientPaymentRequest {
    pub fn normalize(&self) -> Result<NormalizedCreateClientPaymentRequest, AppError> {
        if self.amount <= 0 {
            return Err(AppError::BadRequest(
                "Amount must be greater than zero".into(),
            ));
        }

        if self.expire_seconds < CLIENT_PAYMENT_MIN_EXPIRE_SECONDS
            || self.expire_seconds > CLIENT_PAYMENT_MAX_EXPIRE_SECONDS
        {
            return Err(AppError::BadRequest(format!(
                "Expire seconds must be between {} and {}",
                CLIENT_PAYMENT_MIN_EXPIRE_SECONDS, CLIENT_PAYMENT_MAX_EXPIRE_SECONDS
            )));
        }

        Ok(NormalizedCreateClientPaymentRequest {
            amount: self.amount,
            expire_seconds: self.expire_seconds,
            custom_ref: normalize_optional_string(self.custom_ref.as_deref()),
            merchant_order_id: normalize_optional_string(self.merchant_order_id.as_deref()),
        })
    }
}

pub struct PaymentService {
    repository: Arc<dyn PaymentRepository>,
    provider: Arc<dyn PaymentProviderGateway>,
}

impl PaymentService {
    pub fn new(
        repository: Arc<dyn PaymentRepository>,
        provider: Arc<dyn PaymentProviderGateway>,
    ) -> Self {
        Self {
            repository,
            provider,
        }
    }

    pub async fn create_qris_payment(
        &self,
        context: &StoreApiTokenAuthContext,
        request: NormalizedCreateClientPaymentRequest,
    ) -> Result<ClientPaymentDetail, AppError> {
        let store_profile = self
            .repository
            .find_store_provider_profile(context.store_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Store not found".into()))?;
        self.ensure_provider_username(&store_profile)?;

        let fee_breakdown = payment_fee_breakdown(request.amount);
        let now = Utc::now();
        let expired_at = Some(now + Duration::seconds(request.expire_seconds));

        let created_payment = self
            .repository
            .insert_payment(NewPaymentRecord {
                id: Uuid::new_v4(),
                store_id: context.store_id,
                created_by_user_id: None,
                provider_name: PAYMENT_PROVIDER_NAME.to_string(),
                merchant_order_id: request.merchant_order_id.clone(),
                custom_ref: request.custom_ref.clone(),
                gross_amount: request.amount,
                platform_tx_fee_bps: PAYMENT_PLATFORM_FEE_BPS,
                platform_tx_fee_amount: fee_breakdown.platform_fee_amount,
                store_pending_credit_amount: fee_breakdown.store_pending_amount,
                status: PaymentStatus::Created,
                expired_at,
                created_at: now,
                updated_at: now,
            })
            .await
            .map_err(|error| {
                map_payment_insert_error(error, request.merchant_order_id.is_some())
            })?;

        let provider_result = self
            .provider
            .generate_qris(GenerateQrisRequest {
                username: store_profile.provider_username.clone(),
                amount: request.amount,
                expire_seconds: request.expire_seconds,
                custom_ref: request.custom_ref.clone(),
            })
            .await?;

        let pending_payment = self
            .repository
            .mark_payment_pending(PaymentPendingUpdate {
                payment_id: created_payment.id,
                provider_trx_id: provider_result.provider_trx_id,
                qris_payload: provider_result.qris_payload,
                provider_created_at: None,
                updated_at: Utc::now(),
            })
            .await?;

        Ok(payment_to_detail(pending_payment))
    }

    pub async fn get_payment_detail(
        &self,
        store_id: Uuid,
        payment_id: Uuid,
    ) -> Result<ClientPaymentDetail, AppError> {
        let payment = self
            .repository
            .find_payment_by_id_for_store(store_id, payment_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Payment not found".into()))?;

        Ok(payment_to_detail(payment))
    }

    pub async fn get_payment_status(
        &self,
        store_id: Uuid,
        payment_id: Uuid,
    ) -> Result<ClientPaymentStatusView, AppError> {
        let payment = self
            .repository
            .find_payment_by_id_for_store(store_id, payment_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Payment not found".into()))?;

        Ok(payment_to_status_view(payment))
    }

    fn ensure_provider_username(
        &self,
        store_profile: &StoreProviderProfile,
    ) -> Result<(), AppError> {
        if store_profile.provider_username.trim().is_empty() {
            return Err(AppError::BadRequest(
                "Store provider username is required".into(),
            ));
        }

        Ok(())
    }
}

fn normalize_optional_string(value: Option<&str>) -> Option<String> {
    value
        .map(|inner| inner.trim().to_string())
        .filter(|inner| !inner.is_empty())
}

fn map_payment_insert_error(error: anyhow::Error, has_merchant_order_id: bool) -> AppError {
    if has_merchant_order_id && is_unique_violation(&error) {
        AppError::Conflict("Merchant order id is already in use for this store".into())
    } else {
        error.into()
    }
}

fn is_unique_violation(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<SqlxError>()
        .and_then(|sqlx_error| match sqlx_error {
            SqlxError::Database(database_error) => {
                database_error.code().map(|code| code == "23505")
            }
            _ => None,
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    };

    use async_trait::async_trait;

    use super::*;
    use crate::modules::payments::application::provider::GeneratedQris;
    use crate::modules::payments::domain::entity::Payment;

    #[derive(Default)]
    struct MockPaymentRepository {
        store_profiles: Mutex<HashMap<Uuid, StoreProviderProfile>>,
        payments: Mutex<HashMap<Uuid, Payment>>,
    }

    impl MockPaymentRepository {
        fn insert_store_profile(&self, store_profile: StoreProviderProfile) {
            self.store_profiles
                .lock()
                .unwrap()
                .insert(store_profile.store_id, store_profile);
        }

        fn get_payment(&self, payment_id: Uuid) -> Payment {
            self.payments
                .lock()
                .unwrap()
                .get(&payment_id)
                .unwrap()
                .clone()
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
            if payment.merchant_order_id.is_some()
                && payments.values().any(|existing| {
                    existing.store_id == payment.store_id
                        && existing.merchant_order_id == payment.merchant_order_id
                })
            {
                return Err(SqlxError::Protocol("duplicate merchant order id".into()).into());
            }

            let stored = Payment {
                id: payment.id,
                store_id: payment.store_id,
                created_by_user_id: payment.created_by_user_id,
                provider_name: payment.provider_name,
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
            payments.insert(stored.id, stored.clone());
            Ok(stored)
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
    }

    #[derive(Clone)]
    enum MockProviderMode {
        Success,
        Failure,
    }

    #[derive(Clone)]
    struct MockProvider {
        mode: MockProviderMode,
        calls: Arc<AtomicUsize>,
    }

    impl MockProvider {
        fn success() -> Self {
            Self {
                mode: MockProviderMode::Success,
                calls: Arc::new(AtomicUsize::new(0)),
            }
        }

        fn failure() -> Self {
            Self {
                mode: MockProviderMode::Failure,
                calls: Arc::new(AtomicUsize::new(0)),
            }
        }
    }

    #[async_trait]
    impl PaymentProviderGateway for MockProvider {
        async fn generate_qris(
            &self,
            _request: GenerateQrisRequest,
        ) -> Result<GeneratedQris, AppError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            match self.mode {
                MockProviderMode::Success => Ok(GeneratedQris {
                    provider_trx_id: "trx_123".into(),
                    qris_payload: "qris-payload".into(),
                }),
                MockProviderMode::Failure => Err(AppError::BadRequest(
                    "Provider rejected QRIS generate request: maintenance".into(),
                )),
            }
        }

        async fn check_payment_status(
            &self,
            _request: crate::modules::payments::application::provider::CheckPaymentStatusRequest,
        ) -> Result<crate::modules::payments::application::provider::CheckedPaymentStatus, AppError>
        {
            unreachable!("not used in payment service tests")
        }

        async fn inquiry_bank(
            &self,
            _request: crate::modules::payments::application::provider::InquiryBankRequest,
        ) -> Result<crate::modules::payments::application::provider::InquiryBankResult, AppError>
        {
            unreachable!("not used in payment service tests")
        }

        async fn transfer(
            &self,
            _request: crate::modules::payments::application::provider::TransferRequest,
        ) -> Result<crate::modules::payments::application::provider::TransferResult, AppError>
        {
            unreachable!("not used in payment service tests")
        }

        async fn check_disbursement_status(
            &self,
            _request: crate::modules::payments::application::provider::CheckDisbursementStatusRequest,
        ) -> Result<
            crate::modules::payments::application::provider::CheckedDisbursementStatus,
            AppError,
        > {
            unreachable!("not used in payment service tests")
        }

        async fn get_balance(
            &self,
            _request: crate::modules::payments::application::provider::GetBalanceRequest,
        ) -> Result<
            crate::modules::payments::application::provider::ProviderBalanceSnapshot,
            AppError,
        > {
            unreachable!("not used in payment service tests")
        }
    }

    fn auth_context(store_id: Uuid) -> StoreApiTokenAuthContext {
        StoreApiTokenAuthContext {
            store_id,
            token_id: Uuid::new_v4(),
            token_identifier: "jq_sk_****abcd".into(),
        }
    }

    #[tokio::test]
    async fn create_payment_success_updates_status_and_fee_breakdown() {
        let store_id = Uuid::new_v4();
        let repository = Arc::new(MockPaymentRepository::default());
        repository.insert_store_profile(StoreProviderProfile {
            store_id,
            provider_username: "merchant-user".into(),
        });
        let provider = Arc::new(MockProvider::success());
        let service = PaymentService::new(repository.clone(), provider.clone());

        let payment = service
            .create_qris_payment(
                &auth_context(store_id),
                CreateClientPaymentRequest {
                    amount: 10_001,
                    expire_seconds: 300,
                    custom_ref: Some("  order-01  ".into()),
                    merchant_order_id: Some("  ext-01  ".into()),
                }
                .normalize()
                .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(payment.status, PaymentStatus::Pending);
        assert_eq!(payment.platform_tx_fee_bps, 300);
        assert_eq!(payment.platform_tx_fee_amount, 300);
        assert_eq!(payment.store_pending_credit_amount, 9_701);
        assert_eq!(payment.provider_trx_id.as_deref(), Some("trx_123"));
        assert_eq!(payment.qris_payload.as_deref(), Some("qris-payload"));
        assert_eq!(provider.calls.load(Ordering::SeqCst), 1);

        let stored = repository.get_payment(payment.id);
        assert_eq!(stored.status, PaymentStatus::Pending);
        assert_eq!(stored.merchant_order_id.as_deref(), Some("ext-01"));
        assert_eq!(stored.custom_ref.as_deref(), Some("order-01"));
    }

    #[tokio::test]
    async fn provider_failure_keeps_internal_row_created_without_fake_provider_fields() {
        let store_id = Uuid::new_v4();
        let repository = Arc::new(MockPaymentRepository::default());
        repository.insert_store_profile(StoreProviderProfile {
            store_id,
            provider_username: "merchant-user".into(),
        });
        let provider = Arc::new(MockProvider::failure());
        let service = PaymentService::new(repository.clone(), provider);

        let error = service
            .create_qris_payment(
                &auth_context(store_id),
                CreateClientPaymentRequest {
                    amount: 10_000,
                    expire_seconds: 300,
                    custom_ref: None,
                    merchant_order_id: Some("merchant-01".into()),
                }
                .normalize()
                .unwrap(),
            )
            .await
            .unwrap_err();

        assert!(matches!(error, AppError::BadRequest(_)));

        let payments = repository.payments.lock().unwrap();
        assert_eq!(payments.len(), 1);
        let stored = payments.values().next().unwrap();
        assert_eq!(stored.status, PaymentStatus::Created);
        assert!(stored.provider_trx_id.is_none());
        assert!(stored.qris_payload.is_none());
    }

    #[tokio::test]
    async fn detail_and_status_reads_are_store_scoped_and_do_not_call_provider() {
        let store_a = Uuid::new_v4();
        let store_b = Uuid::new_v4();
        let repository = Arc::new(MockPaymentRepository::default());
        repository.insert_store_profile(StoreProviderProfile {
            store_id: store_a,
            provider_username: "merchant-a".into(),
        });
        let provider = Arc::new(MockProvider::success());
        let service = PaymentService::new(repository.clone(), provider.clone());

        let created = service
            .create_qris_payment(
                &auth_context(store_a),
                CreateClientPaymentRequest {
                    amount: 50_000,
                    expire_seconds: 300,
                    custom_ref: None,
                    merchant_order_id: None,
                }
                .normalize()
                .unwrap(),
            )
            .await
            .unwrap();

        let provider_calls_after_create = provider.calls.load(Ordering::SeqCst);
        let detail = service
            .get_payment_detail(store_a, created.id)
            .await
            .unwrap();
        let status = service
            .get_payment_status(store_a, created.id)
            .await
            .unwrap();

        assert_eq!(detail.id, created.id);
        assert_eq!(status.id, created.id);
        assert_eq!(
            provider.calls.load(Ordering::SeqCst),
            provider_calls_after_create
        );

        let cross_store_detail = service.get_payment_detail(store_b, created.id).await;
        let cross_store_status = service.get_payment_status(store_b, created.id).await;
        assert!(matches!(
            cross_store_detail.unwrap_err(),
            AppError::NotFound(_)
        ));
        assert!(matches!(
            cross_store_status.unwrap_err(),
            AppError::NotFound(_)
        ));
    }

    #[test]
    fn normalize_rejects_invalid_amount_and_expiry_range() {
        let invalid_amount = CreateClientPaymentRequest {
            amount: 0,
            expire_seconds: 300,
            custom_ref: None,
            merchant_order_id: None,
        }
        .normalize()
        .unwrap_err();
        assert!(matches!(invalid_amount, AppError::BadRequest(_)));

        let invalid_expiry = CreateClientPaymentRequest {
            amount: 10_000,
            expire_seconds: 30,
            custom_ref: None,
            merchant_order_id: None,
        }
        .normalize()
        .unwrap_err();
        assert!(matches!(invalid_expiry, AppError::BadRequest(_)));
    }
}
