use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{types::Json, FromRow};
use uuid::Uuid;

use crate::infrastructure::db::DbPool;
use crate::modules::payments::domain::entity::{
    NewPaymentIdempotencyRecord, NewPaymentRecord, Payment, PaymentIdempotencyRecord,
    PaymentIdempotencyStatus, PaymentPendingUpdate, PaymentStatus, StoreProviderProfile,
};
use crate::modules::payments::domain::repository::{
    PaymentIdempotencyRepository, PaymentRepository,
};

pub struct SqlxPaymentRepository {
    db: DbPool,
}

impl SqlxPaymentRepository {
    pub fn new(db: DbPool) -> Self {
        Self { db }
    }
}

#[async_trait]
impl PaymentRepository for SqlxPaymentRepository {
    async fn find_store_provider_profile(
        &self,
        store_id: Uuid,
    ) -> anyhow::Result<Option<StoreProviderProfile>> {
        let row = sqlx::query_as::<_, StoreProviderProfileRow>(
            r#"
            SELECT id, provider_username
            FROM stores
            WHERE id = $1
              AND deleted_at IS NULL
            "#,
        )
        .bind(store_id)
        .fetch_optional(&self.db)
        .await?;

        Ok(row.map(|record| StoreProviderProfile {
            store_id: record.id,
            provider_username: record.provider_username.unwrap_or_default(),
        }))
    }

    async fn insert_payment(&self, payment: NewPaymentRecord) -> anyhow::Result<Payment> {
        let row = sqlx::query_as::<_, PaymentRow>(
            r#"
            INSERT INTO payments (
                id,
                store_id,
                created_by_user_id,
                provider_name,
                merchant_order_id,
                custom_ref,
                gross_amount,
                platform_tx_fee_bps,
                platform_tx_fee_amount,
                store_pending_credit_amount,
                status,
                expired_at,
                created_at,
                updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            RETURNING
                id,
                store_id,
                created_by_user_id,
                provider_name,
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
            "#,
        )
        .bind(payment.id)
        .bind(payment.store_id)
        .bind(payment.created_by_user_id)
        .bind(payment.provider_name)
        .bind(payment.merchant_order_id)
        .bind(payment.custom_ref)
        .bind(payment.gross_amount)
        .bind(payment.platform_tx_fee_bps)
        .bind(payment.platform_tx_fee_amount)
        .bind(payment.store_pending_credit_amount)
        .bind(payment.status.to_string())
        .bind(payment.expired_at)
        .bind(payment.created_at)
        .bind(payment.updated_at)
        .fetch_one(&self.db)
        .await?;

        Ok(row.into())
    }

    async fn mark_payment_pending(&self, update: PaymentPendingUpdate) -> anyhow::Result<Payment> {
        let row = sqlx::query_as::<_, PaymentRow>(
            r#"
            UPDATE payments
            SET
                provider_trx_id = $2,
                qris_payload = $3,
                provider_created_at = $4,
                status = 'pending',
                updated_at = $5
            WHERE id = $1
            RETURNING
                id,
                store_id,
                created_by_user_id,
                provider_name,
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
            "#,
        )
        .bind(update.payment_id)
        .bind(update.provider_trx_id)
        .bind(update.qris_payload)
        .bind(update.provider_created_at)
        .bind(update.updated_at)
        .fetch_one(&self.db)
        .await?;

        Ok(row.into())
    }

    async fn find_payment_by_id_for_store(
        &self,
        store_id: Uuid,
        payment_id: Uuid,
    ) -> anyhow::Result<Option<Payment>> {
        let row = sqlx::query_as::<_, PaymentRow>(
            r#"
            SELECT
                id,
                store_id,
                created_by_user_id,
                provider_name,
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
            FROM payments
            WHERE id = $1
              AND store_id = $2
            "#,
        )
        .bind(payment_id)
        .bind(store_id)
        .fetch_optional(&self.db)
        .await?;

        Ok(row.map(Into::into))
    }
}

#[async_trait]
impl PaymentIdempotencyRepository for SqlxPaymentRepository {
    async fn find_by_key(
        &self,
        store_id: Uuid,
        idempotency_key: &str,
    ) -> anyhow::Result<Option<PaymentIdempotencyRecord>> {
        let row = sqlx::query_as::<_, PaymentIdempotencyRow>(
            r#"
            SELECT
                id,
                store_id,
                idempotency_key,
                request_hash,
                status,
                response_status_code,
                response_body_json,
                payment_id,
                created_at,
                completed_at,
                updated_at
            FROM payment_idempotency_keys
            WHERE store_id = $1
              AND idempotency_key = $2
            "#,
        )
        .bind(store_id)
        .bind(idempotency_key)
        .fetch_optional(&self.db)
        .await?;

        Ok(row.map(Into::into))
    }

    async fn insert_pending(
        &self,
        record: NewPaymentIdempotencyRecord,
    ) -> anyhow::Result<PaymentIdempotencyRecord> {
        let row = sqlx::query_as::<_, PaymentIdempotencyRow>(
            r#"
            INSERT INTO payment_idempotency_keys (
                id,
                store_id,
                idempotency_key,
                request_hash,
                status,
                created_at,
                updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING
                id,
                store_id,
                idempotency_key,
                request_hash,
                status,
                response_status_code,
                response_body_json,
                payment_id,
                created_at,
                completed_at,
                updated_at
            "#,
        )
        .bind(record.id)
        .bind(record.store_id)
        .bind(record.idempotency_key)
        .bind(record.request_hash)
        .bind(record.status.to_string())
        .bind(record.created_at)
        .bind(record.updated_at)
        .fetch_one(&self.db)
        .await?;

        Ok(row.into())
    }

    async fn complete(
        &self,
        store_id: Uuid,
        idempotency_key: &str,
        response_status_code: i32,
        response_body_json: Value,
        payment_id: Option<Uuid>,
        completed_at: DateTime<Utc>,
    ) -> anyhow::Result<PaymentIdempotencyRecord> {
        let row = sqlx::query_as::<_, PaymentIdempotencyRow>(
            r#"
            UPDATE payment_idempotency_keys
            SET
                status = 'completed',
                response_status_code = $3,
                response_body_json = $4,
                payment_id = $5,
                completed_at = $6,
                updated_at = $6
            WHERE store_id = $1
              AND idempotency_key = $2
            RETURNING
                id,
                store_id,
                idempotency_key,
                request_hash,
                status,
                response_status_code,
                response_body_json,
                payment_id,
                created_at,
                completed_at,
                updated_at
            "#,
        )
        .bind(store_id)
        .bind(idempotency_key)
        .bind(response_status_code)
        .bind(Json(response_body_json))
        .bind(payment_id)
        .bind(completed_at)
        .fetch_one(&self.db)
        .await?;

        Ok(row.into())
    }
}

#[derive(Debug, FromRow)]
struct StoreProviderProfileRow {
    id: Uuid,
    provider_username: Option<String>,
}

#[derive(Debug, FromRow)]
struct PaymentRow {
    id: Uuid,
    store_id: Uuid,
    created_by_user_id: Option<Uuid>,
    provider_name: String,
    provider_trx_id: Option<String>,
    provider_rrn: Option<String>,
    merchant_order_id: Option<String>,
    custom_ref: Option<String>,
    gross_amount: i64,
    platform_tx_fee_bps: i32,
    platform_tx_fee_amount: i64,
    store_pending_credit_amount: i64,
    status: String,
    qris_payload: Option<String>,
    expired_at: Option<DateTime<Utc>>,
    provider_created_at: Option<DateTime<Utc>>,
    provider_finished_at: Option<DateTime<Utc>>,
    finalized_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<PaymentRow> for Payment {
    fn from(value: PaymentRow) -> Self {
        Self {
            id: value.id,
            store_id: value.store_id,
            created_by_user_id: value.created_by_user_id,
            provider_name: value.provider_name,
            provider_trx_id: value.provider_trx_id,
            provider_rrn: value.provider_rrn,
            merchant_order_id: value.merchant_order_id,
            custom_ref: value.custom_ref,
            gross_amount: value.gross_amount,
            platform_tx_fee_bps: value.platform_tx_fee_bps,
            platform_tx_fee_amount: value.platform_tx_fee_amount,
            store_pending_credit_amount: value.store_pending_credit_amount,
            status: PaymentStatus::from(value.status),
            qris_payload: value.qris_payload,
            expired_at: value.expired_at,
            provider_created_at: value.provider_created_at,
            provider_finished_at: value.provider_finished_at,
            finalized_at: value.finalized_at,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

#[derive(Debug, FromRow)]
struct PaymentIdempotencyRow {
    id: Uuid,
    store_id: Uuid,
    idempotency_key: String,
    request_hash: String,
    status: String,
    response_status_code: Option<i32>,
    response_body_json: Option<Json<Value>>,
    payment_id: Option<Uuid>,
    created_at: DateTime<Utc>,
    completed_at: Option<DateTime<Utc>>,
    updated_at: DateTime<Utc>,
}

impl From<PaymentIdempotencyRow> for PaymentIdempotencyRecord {
    fn from(value: PaymentIdempotencyRow) -> Self {
        Self {
            id: value.id,
            store_id: value.store_id,
            idempotency_key: value.idempotency_key,
            request_hash: value.request_hash,
            status: PaymentIdempotencyStatus::from(value.status),
            response_status_code: value.response_status_code,
            response_body_json: value.response_body_json.map(|payload| payload.0),
            payment_id: value.payment_id,
            created_at: value.created_at,
            completed_at: value.completed_at,
            updated_at: value.updated_at,
        }
    }
}
