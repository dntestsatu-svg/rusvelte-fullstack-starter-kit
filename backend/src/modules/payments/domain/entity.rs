use std::fmt::{Display, Formatter};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const PAYMENT_PROVIDER_NAME: &str = "qris_otomatis_vip";
pub const PAYMENT_PLATFORM_FEE_BPS: i32 = 300;
pub const CLIENT_PAYMENT_MIN_EXPIRE_SECONDS: i64 = 60;
pub const CLIENT_PAYMENT_MAX_EXPIRE_SECONDS: i64 = 3600;
pub const CLIENT_PAYMENT_CREATE_RATE_LIMIT: u64 = 5;
pub const CLIENT_PAYMENT_CREATE_RATE_WINDOW_SECONDS: u64 = 60;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PaymentStatus {
    Created,
    Pending,
    Success,
    Failed,
    Expired,
}

impl Display for PaymentStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            Self::Created => "created",
            Self::Pending => "pending",
            Self::Success => "success",
            Self::Failed => "failed",
            Self::Expired => "expired",
        };

        f.write_str(value)
    }
}

impl From<String> for PaymentStatus {
    fn from(value: String) -> Self {
        match value.as_str() {
            "created" => Self::Created,
            "pending" => Self::Pending,
            "success" => Self::Success,
            "failed" => Self::Failed,
            "expired" => Self::Expired,
            _ => Self::Created,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Payment {
    pub id: Uuid,
    pub store_id: Uuid,
    pub created_by_user_id: Option<Uuid>,
    pub provider_name: String,
    pub provider_trx_id: Option<String>,
    pub provider_rrn: Option<String>,
    pub merchant_order_id: Option<String>,
    pub custom_ref: Option<String>,
    pub gross_amount: i64,
    pub platform_tx_fee_bps: i32,
    pub platform_tx_fee_amount: i64,
    pub store_pending_credit_amount: i64,
    pub status: PaymentStatus,
    pub qris_payload: Option<String>,
    pub expired_at: Option<DateTime<Utc>>,
    pub provider_created_at: Option<DateTime<Utc>>,
    pub provider_finished_at: Option<DateTime<Utc>>,
    pub finalized_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewPaymentRecord {
    pub id: Uuid,
    pub store_id: Uuid,
    pub created_by_user_id: Option<Uuid>,
    pub provider_name: String,
    pub merchant_order_id: Option<String>,
    pub custom_ref: Option<String>,
    pub gross_amount: i64,
    pub platform_tx_fee_bps: i32,
    pub platform_tx_fee_amount: i64,
    pub store_pending_credit_amount: i64,
    pub status: PaymentStatus,
    pub expired_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct PaymentPendingUpdate {
    pub payment_id: Uuid,
    pub provider_trx_id: String,
    pub qris_payload: String,
    pub provider_created_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoreProviderProfile {
    pub store_id: Uuid,
    pub provider_username: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClientPaymentDetail {
    pub id: Uuid,
    pub store_id: Uuid,
    pub provider_name: String,
    pub provider_trx_id: Option<String>,
    pub merchant_order_id: Option<String>,
    pub custom_ref: Option<String>,
    pub gross_amount: i64,
    pub platform_tx_fee_bps: i32,
    pub platform_tx_fee_amount: i64,
    pub store_pending_credit_amount: i64,
    pub status: PaymentStatus,
    pub qris_payload: Option<String>,
    pub expired_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClientPaymentStatusView {
    pub id: Uuid,
    pub store_id: Uuid,
    pub gross_amount: i64,
    pub merchant_order_id: Option<String>,
    pub custom_ref: Option<String>,
    pub status: PaymentStatus,
    pub provider_trx_id: Option<String>,
    pub expired_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PaymentIdempotencyStatus {
    Pending,
    Completed,
}

impl Display for PaymentIdempotencyStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            Self::Pending => "pending",
            Self::Completed => "completed",
        };

        f.write_str(value)
    }
}

impl From<String> for PaymentIdempotencyStatus {
    fn from(value: String) -> Self {
        match value.as_str() {
            "completed" => Self::Completed,
            _ => Self::Pending,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PaymentIdempotencyRecord {
    pub id: Uuid,
    pub store_id: Uuid,
    pub idempotency_key: String,
    pub request_hash: String,
    pub status: PaymentIdempotencyStatus,
    pub response_status_code: Option<i32>,
    pub response_body_json: Option<serde_json::Value>,
    pub payment_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewPaymentIdempotencyRecord {
    pub id: Uuid,
    pub store_id: Uuid,
    pub idempotency_key: String,
    pub request_hash: String,
    pub status: PaymentIdempotencyStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub fn payment_to_detail(payment: Payment) -> ClientPaymentDetail {
    ClientPaymentDetail {
        id: payment.id,
        store_id: payment.store_id,
        provider_name: payment.provider_name,
        provider_trx_id: payment.provider_trx_id,
        merchant_order_id: payment.merchant_order_id,
        custom_ref: payment.custom_ref,
        gross_amount: payment.gross_amount,
        platform_tx_fee_bps: payment.platform_tx_fee_bps,
        platform_tx_fee_amount: payment.platform_tx_fee_amount,
        store_pending_credit_amount: payment.store_pending_credit_amount,
        status: payment.status,
        qris_payload: payment.qris_payload,
        expired_at: payment.expired_at,
        created_at: payment.created_at,
        updated_at: payment.updated_at,
    }
}

pub fn payment_to_status_view(payment: Payment) -> ClientPaymentStatusView {
    ClientPaymentStatusView {
        id: payment.id,
        store_id: payment.store_id,
        gross_amount: payment.gross_amount,
        merchant_order_id: payment.merchant_order_id,
        custom_ref: payment.custom_ref,
        status: payment.status,
        provider_trx_id: payment.provider_trx_id,
        expired_at: payment.expired_at,
        created_at: payment.created_at,
        updated_at: payment.updated_at,
    }
}
