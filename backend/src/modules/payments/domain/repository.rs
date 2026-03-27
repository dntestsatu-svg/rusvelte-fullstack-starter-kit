use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::modules::payments::domain::entity::{
    DashboardPaymentDetail, DashboardPaymentDistribution, DashboardPaymentSummary,
    NewPaymentIdempotencyRecord, NewPaymentRecord, NewProviderWebhookEventRecord, Payment,
    PaymentIdempotencyRecord, PaymentPendingUpdate, PaymentWebhookFinalizeCommand,
    PaymentWebhookFinalizeOutcome, PaymentWebhookTarget, ProviderWebhookEvent, StoreProviderProfile,
};

#[async_trait]
pub trait PaymentRepository: Send + Sync {
    async fn find_store_provider_profile(
        &self,
        store_id: Uuid,
    ) -> anyhow::Result<Option<StoreProviderProfile>>;

    async fn insert_payment(&self, payment: NewPaymentRecord) -> anyhow::Result<Payment>;

    async fn mark_payment_pending(&self, update: PaymentPendingUpdate) -> anyhow::Result<Payment>;

    async fn find_payment_by_id_for_store(
        &self,
        store_id: Uuid,
        payment_id: Uuid,
    ) -> anyhow::Result<Option<Payment>>;

    async fn list_dashboard_payments(
        &self,
        limit: i64,
        offset: i64,
        search: Option<&str>,
        status: Option<&str>,
        user_scope: Option<Uuid>,
        global_access: bool,
    ) -> anyhow::Result<Vec<DashboardPaymentSummary>>;

    async fn count_dashboard_payments(
        &self,
        search: Option<&str>,
        status: Option<&str>,
        user_scope: Option<Uuid>,
        global_access: bool,
    ) -> anyhow::Result<i64>;

    async fn find_dashboard_payment_by_id(
        &self,
        payment_id: Uuid,
        user_scope: Option<Uuid>,
        global_access: bool,
    ) -> anyhow::Result<Option<DashboardPaymentDetail>>;

    async fn count_dashboard_payment_distribution(
        &self,
        user_scope: Option<Uuid>,
        global_access: bool,
    ) -> anyhow::Result<DashboardPaymentDistribution>;

    async fn find_payment_by_provider_trx_id(
        &self,
        provider_name: &str,
        provider_trx_id: &str,
    ) -> anyhow::Result<Option<PaymentWebhookTarget>>;

    async fn insert_provider_webhook_event(
        &self,
        event: NewProviderWebhookEventRecord,
    ) -> anyhow::Result<ProviderWebhookEvent>;

    async fn mark_provider_webhook_event_result(
        &self,
        event_id: Uuid,
        is_verified: bool,
        verification_reason: Option<&str>,
        is_processed: bool,
        processing_result: Option<&str>,
        processed_at: Option<DateTime<Utc>>,
    ) -> anyhow::Result<ProviderWebhookEvent>;

    async fn finalize_payment_from_webhook(
        &self,
        command: PaymentWebhookFinalizeCommand,
    ) -> anyhow::Result<PaymentWebhookFinalizeOutcome>;
}

#[async_trait]
pub trait PaymentIdempotencyRepository: Send + Sync {
    async fn find_by_key(
        &self,
        store_id: Uuid,
        idempotency_key: &str,
    ) -> anyhow::Result<Option<PaymentIdempotencyRecord>>;

    async fn insert_pending(
        &self,
        record: NewPaymentIdempotencyRecord,
    ) -> anyhow::Result<PaymentIdempotencyRecord>;

    async fn complete(
        &self,
        store_id: Uuid,
        idempotency_key: &str,
        response_status_code: i32,
        response_body_json: serde_json::Value,
        payment_id: Option<Uuid>,
        completed_at: DateTime<Utc>,
    ) -> anyhow::Result<PaymentIdempotencyRecord>;
}
