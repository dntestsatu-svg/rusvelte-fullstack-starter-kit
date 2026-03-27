use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::modules::payments::domain::entity::{
    NewPaymentIdempotencyRecord, NewPaymentRecord, Payment, PaymentIdempotencyRecord,
    PaymentPendingUpdate, StoreProviderProfile,
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
