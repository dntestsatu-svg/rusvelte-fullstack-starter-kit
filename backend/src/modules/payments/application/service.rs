use std::sync::Arc;

use chrono::{Duration, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::Sha256;
use sqlx::Error as SqlxError;
use uuid::Uuid;

use crate::modules::payments::application::provider::{
    GenerateQrisRequest, GetBalanceRequest, PaymentProviderGateway, ProviderBalanceSnapshot,
};
use crate::modules::payments::domain::entity::{
    payment_to_detail, payment_to_status_view, ClientPaymentDetail, ClientPaymentStatusView,
    DashboardPaymentDetail, DashboardPaymentDistribution, DashboardPaymentSummary, NewPaymentRecord,
    NewProviderWebhookEventRecord, Payment, PaymentPendingUpdate, PaymentStatus,
    PaymentWebhookFinalizeCommand, PaymentWebhookFinalizeOutcomeKind,
    PaymentWebhookStatus, PendingCallbackDelivery, ProviderWebhookKind, StoreProviderProfile,
    CLIENT_PAYMENT_MAX_EXPIRE_SECONDS, CLIENT_PAYMENT_MIN_EXPIRE_SECONDS,
    PAYMENT_PLATFORM_FEE_BPS, PAYMENT_PROVIDER_NAME,
};
use crate::modules::payments::domain::repository::PaymentRepository;
use crate::modules::store_tokens::domain::entity::StoreApiTokenAuthContext;
use crate::shared::auth::{has_capability, AuthenticatedUser, Capability};
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

#[derive(Debug, Clone, Default, Deserialize)]
pub struct DashboardPaymentListFilters {
    pub search: Option<String>,
    pub status: Option<PaymentStatus>,
}

#[derive(Debug, Clone)]
pub enum ProviderWebhookInput {
    Payment(PaymentWebhookInput),
    Payout(PayoutWebhookInput),
    Unknown(UnknownWebhookInput),
}

#[derive(Debug, Clone)]
pub struct PaymentWebhookInput {
    pub merchant_id: String,
    pub provider_trx_id: String,
    pub terminal_id: String,
    pub custom_ref: Option<String>,
    pub rrn: Option<String>,
    pub amount: i64,
    pub status: PaymentWebhookStatus,
    pub provider_created_at: Option<chrono::DateTime<Utc>>,
    pub provider_finished_at: Option<chrono::DateTime<Utc>>,
    pub raw_payload: Value,
}

#[derive(Debug, Clone)]
pub struct PayoutWebhookInput {
    pub merchant_id: Option<String>,
    pub partner_ref_no: Option<String>,
    pub raw_payload: Value,
}

#[derive(Debug, Clone)]
pub struct UnknownWebhookInput {
    pub merchant_id: Option<String>,
    pub provider_trx_id: Option<String>,
    pub partner_ref_no: Option<String>,
    pub raw_payload: Value,
}

#[derive(Debug, Clone)]
pub struct ProcessProviderWebhookResult {
    pub response_status: bool,
    pub payment: Option<Payment>,
    pub notification_user_ids: Vec<Uuid>,
    pub publish_payment_event: bool,
    pub publish_notification_event: bool,
    pub callback_enqueued: bool,
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
                provider_terminal_id: store_profile.provider_username.clone(),
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

    pub async fn list_dashboard_payments(
        &self,
        limit: i64,
        offset: i64,
        filters: DashboardPaymentListFilters,
        actor: &AuthenticatedUser,
    ) -> Result<Vec<DashboardPaymentSummary>, AppError> {
        ensure_any_payment_read(actor)?;
        let (user_scope, global_access) = payment_scope(actor);

        self.repository
            .list_dashboard_payments(
                limit,
                offset,
                filters.search.as_deref(),
                filters.status.as_ref().map(|status| status.to_string()).as_deref(),
                user_scope,
                global_access,
            )
            .await
            .map_err(Into::into)
    }

    pub async fn count_dashboard_payments(
        &self,
        filters: DashboardPaymentListFilters,
        actor: &AuthenticatedUser,
    ) -> Result<i64, AppError> {
        ensure_any_payment_read(actor)?;
        let (user_scope, global_access) = payment_scope(actor);

        self.repository
            .count_dashboard_payments(
                filters.search.as_deref(),
                filters.status.as_ref().map(|status| status.to_string()).as_deref(),
                user_scope,
                global_access,
            )
            .await
            .map_err(Into::into)
    }

    pub async fn get_dashboard_payment(
        &self,
        payment_id: Uuid,
        actor: &AuthenticatedUser,
    ) -> Result<DashboardPaymentDetail, AppError> {
        ensure_any_payment_read(actor)?;
        let (user_scope, global_access) = payment_scope(actor);
        let payment = self
            .repository
            .find_dashboard_payment_by_id(payment_id, user_scope, global_access)
            .await?
            .ok_or_else(|| AppError::NotFound("Payment not found".into()))?;

        Ok(payment)
    }

    pub async fn get_dashboard_payment_distribution(
        &self,
        actor: &AuthenticatedUser,
    ) -> Result<DashboardPaymentDistribution, AppError> {
        ensure_any_payment_read(actor)?;
        let (user_scope, global_access) = payment_scope(actor);

        self.repository
            .count_dashboard_payment_distribution(user_scope, global_access)
            .await
            .map_err(Into::into)
    }

    pub async fn get_provider_balance_snapshot(&self) -> Result<ProviderBalanceSnapshot, AppError> {
        self.provider.get_balance(GetBalanceRequest).await
    }

    pub async fn process_provider_webhook(
        &self,
        webhook: ProviderWebhookInput,
        expected_merchant_id: &str,
    ) -> Result<ProcessProviderWebhookResult, AppError> {
        let now = Utc::now();

        match webhook {
            ProviderWebhookInput::Payment(payment) => {
                self.process_payment_webhook(payment, expected_merchant_id, now)
                    .await
            }
            ProviderWebhookInput::Payout(payout) => {
                let event = self
                    .repository
                    .insert_provider_webhook_event(NewProviderWebhookEventRecord {
                        id: Uuid::new_v4(),
                        provider_name: PAYMENT_PROVIDER_NAME.to_string(),
                        webhook_kind: ProviderWebhookKind::Payout,
                        merchant_id: payout.merchant_id.clone(),
                        provider_trx_id: None,
                        partner_ref_no: payout.partner_ref_no.clone(),
                        payload_json: payout.raw_payload,
                        created_at: now,
                    })
                    .await?;

                let is_verified = payout
                    .merchant_id
                    .as_deref()
                    .map(|value| value == expected_merchant_id)
                    .unwrap_or(false);
                let verification_reason = if is_verified {
                    None
                } else {
                    Some("Merchant id mismatch")
                };
                let processing_result = if is_verified {
                    Some("payout_webhook_recorded")
                } else {
                    Some("invalid_payout_webhook")
                };

                self.repository
                    .mark_provider_webhook_event_result(
                        event.id,
                        is_verified,
                        verification_reason,
                        true,
                        processing_result,
                        Some(now),
                    )
                    .await?;

                Ok(ProcessProviderWebhookResult {
                    response_status: is_verified,
                    payment: None,
                    notification_user_ids: vec![],
                    publish_payment_event: false,
                    publish_notification_event: false,
                    callback_enqueued: false,
                })
            }
            ProviderWebhookInput::Unknown(unknown) => {
                let event = self
                    .repository
                    .insert_provider_webhook_event(NewProviderWebhookEventRecord {
                        id: Uuid::new_v4(),
                        provider_name: PAYMENT_PROVIDER_NAME.to_string(),
                        webhook_kind: ProviderWebhookKind::Unknown,
                        merchant_id: unknown.merchant_id,
                        provider_trx_id: unknown.provider_trx_id,
                        partner_ref_no: unknown.partner_ref_no,
                        payload_json: unknown.raw_payload,
                        created_at: now,
                    })
                    .await?;

                self.repository
                    .mark_provider_webhook_event_result(
                        event.id,
                        false,
                        Some("Unsupported webhook payload shape"),
                        true,
                        Some("unsupported_webhook_payload"),
                        Some(now),
                    )
                    .await?;

                Ok(ProcessProviderWebhookResult {
                    response_status: false,
                    payment: None,
                    notification_user_ids: vec![],
                    publish_payment_event: false,
                    publish_notification_event: false,
                    callback_enqueued: false,
                })
            }
        }
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

    async fn process_payment_webhook(
        &self,
        payment: PaymentWebhookInput,
        expected_merchant_id: &str,
        now: chrono::DateTime<Utc>,
    ) -> Result<ProcessProviderWebhookResult, AppError> {
        let event = self
            .repository
            .insert_provider_webhook_event(NewProviderWebhookEventRecord {
                id: Uuid::new_v4(),
                provider_name: PAYMENT_PROVIDER_NAME.to_string(),
                webhook_kind: ProviderWebhookKind::Payment,
                merchant_id: Some(payment.merchant_id.clone()),
                provider_trx_id: Some(payment.provider_trx_id.clone()),
                partner_ref_no: None,
                payload_json: payment.raw_payload.clone(),
                created_at: now,
            })
            .await?;

        if payment.merchant_id != expected_merchant_id {
            return self
                .reject_webhook_event(event.id, "Merchant id mismatch", "invalid_merchant_id", now)
                .await;
        }

        let Some(target) = self
            .repository
            .find_payment_by_provider_trx_id(PAYMENT_PROVIDER_NAME, &payment.provider_trx_id)
            .await?
        else {
            return self
                .reject_webhook_event(
                    event.id,
                    "Provider transaction not found",
                    "invalid_provider_trx_id",
                    now,
                )
                .await;
        };

        if target.payment.provider_terminal_id.as_deref() != Some(payment.terminal_id.as_str()) {
            return self
                .reject_webhook_event(
                    event.id,
                    "Terminal id mismatch",
                    "invalid_terminal_id",
                    now,
                )
                .await;
        }

        if target.payment.gross_amount != payment.amount {
            return self
                .reject_webhook_event(
                    event.id,
                    "Amount mismatch",
                    "invalid_amount",
                    now,
                )
                .await;
        }

        if !custom_ref_matches(target.payment.custom_ref.as_deref(), payment.custom_ref.as_deref()) {
            return self
                .reject_webhook_event(
                    event.id,
                    "Custom ref mismatch",
                    "invalid_custom_ref",
                    now,
                )
                .await;
        }

        let callback_delivery = build_callback_delivery(
            &target,
            &target.payment,
            &payment.status,
            now,
        )?;
        let (notification_type, notification_title, notification_body) =
            notification_copy(&target.store_name, &target.payment, &payment.status);
        let outcome = self
            .repository
            .finalize_payment_from_webhook(PaymentWebhookFinalizeCommand {
                webhook_event_id: event.id,
                payment_id: target.payment.id,
                final_status: payment.status.to_payment_status(),
                provider_rrn: normalize_optional_string(payment.rrn.as_deref()),
                provider_finished_at: payment.provider_finished_at.or(payment.provider_created_at),
                payload_json: payment.raw_payload,
                notification_type,
                notification_title,
                notification_body,
                callback_delivery,
                processed_at: now,
            })
            .await?;

        let publish_payment_event = outcome.kind == PaymentWebhookFinalizeOutcomeKind::Finalized
            && outcome.payment.is_some();
        let publish_notification_event =
            publish_payment_event && !outcome.notification_user_ids.is_empty();

        Ok(ProcessProviderWebhookResult {
            response_status: true,
            payment: outcome.payment,
            notification_user_ids: outcome.notification_user_ids,
            publish_payment_event,
            publish_notification_event,
            callback_enqueued: outcome.callback_enqueued,
        })
    }

    async fn reject_webhook_event(
        &self,
        event_id: Uuid,
        verification_reason: &str,
        processing_result: &str,
        processed_at: chrono::DateTime<Utc>,
    ) -> Result<ProcessProviderWebhookResult, AppError> {
        self.repository
            .mark_provider_webhook_event_result(
                event_id,
                false,
                Some(verification_reason),
                true,
                Some(processing_result),
                Some(processed_at),
            )
            .await?;

        Ok(ProcessProviderWebhookResult {
            response_status: false,
            payment: None,
            notification_user_ids: vec![],
            publish_payment_event: false,
            publish_notification_event: false,
            callback_enqueued: false,
        })
    }
}

fn normalize_optional_string(value: Option<&str>) -> Option<String> {
    value
        .map(|inner| inner.trim().to_string())
        .filter(|inner| !inner.is_empty())
}

fn ensure_any_payment_read(actor: &AuthenticatedUser) -> Result<(), AppError> {
    if has_capability(actor, Capability::PaymentReadGlobal, None)
        || has_capability(actor, Capability::PaymentRead, None)
        || !actor.memberships.is_empty()
    {
        Ok(())
    } else {
        Err(AppError::Forbidden(
            "You do not have permission to view payments".into(),
        ))
    }
}

fn payment_scope(actor: &AuthenticatedUser) -> (Option<Uuid>, bool) {
    if has_capability(actor, Capability::PaymentReadGlobal, None) {
        (Some(actor.user_id), true)
    } else {
        (Some(actor.user_id), false)
    }
}

fn custom_ref_matches(expected: Option<&str>, actual: Option<&str>) -> bool {
    match (normalize_optional_string(expected), normalize_optional_string(actual)) {
        (Some(expected), Some(actual)) => expected == actual,
        (Some(_), None) => false,
        (None, _) => true,
    }
}

fn notification_copy(
    store_name: &str,
    payment: &Payment,
    status: &PaymentWebhookStatus,
) -> (String, String, String) {
    match status {
        PaymentWebhookStatus::Success => (
            "payment_success".into(),
            format!("Payment success for {store_name}"),
            format!(
                "Payment {} succeeded and Rp {} moved into pending balance.",
                payment.id, payment.store_pending_credit_amount
            ),
        ),
        PaymentWebhookStatus::Failed => (
            "payment_failed".into(),
            format!("Payment failed for {store_name}"),
            format!("Payment {} failed at provider side.", payment.id),
        ),
        PaymentWebhookStatus::Expired => (
            "payment_expired".into(),
            format!("Payment expired for {store_name}"),
            format!("Payment {} expired before completion.", payment.id),
        ),
    }
}

fn build_callback_delivery(
    target: &crate::modules::payments::domain::entity::PaymentWebhookTarget,
    payment: &Payment,
    status: &PaymentWebhookStatus,
    now: chrono::DateTime<Utc>,
) -> Result<Option<PendingCallbackDelivery>, AppError> {
    let Some(target_url) = normalize_optional_string(target.callback_url.as_deref()) else {
        return Ok(None);
    };
    let Some(secret) = normalize_optional_string(target.callback_secret.as_deref()) else {
        return Ok(None);
    };

    let payload = serde_json::json!({
        "event": format!("payment.{status}"),
        "payment_id": payment.id,
        "store_id": payment.store_id,
        "status": status.to_payment_status(),
        "provider_trx_id": payment.provider_trx_id,
        "merchant_order_id": payment.merchant_order_id,
        "custom_ref": payment.custom_ref,
        "gross_amount": payment.gross_amount,
        "platform_tx_fee_amount": payment.platform_tx_fee_amount,
        "store_pending_credit_amount": payment.store_pending_credit_amount,
        "finalized_at": now,
        "timestamp": now,
    });
    let payload_json = serde_json::to_string(&payload)
        .map_err(|error| AppError::Internal(anyhow::anyhow!(error.to_string())))?;
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
        .map_err(|error| AppError::Internal(anyhow::anyhow!(error.to_string())))?;
    mac.update(now.to_rfc3339().as_bytes());
    mac.update(payload_json.as_bytes());
    let signature = mac
        .finalize()
        .into_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();

    Ok(Some(PendingCallbackDelivery {
        event_type: format!("payment.{status}"),
        target_url,
        signature,
    }))
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
    use crate::modules::payments::domain::entity::{
        DashboardPaymentDetail, DashboardPaymentDistribution, DashboardPaymentSummary,
        NewProviderWebhookEventRecord, Payment, PaymentWebhookFinalizeCommand,
    };

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
        ) -> anyhow::Result<DashboardPaymentDistribution> {
            Ok(DashboardPaymentDistribution {
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
            unreachable!("not used in payment service tests")
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
            unreachable!("not used in payment service tests")
        }

        async fn finalize_payment_from_webhook(
            &self,
            _command: PaymentWebhookFinalizeCommand,
        ) -> anyhow::Result<crate::modules::payments::domain::entity::PaymentWebhookFinalizeOutcome>
        {
            unreachable!("not used in payment service tests")
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
