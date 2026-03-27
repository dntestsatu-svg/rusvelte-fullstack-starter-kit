use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::shared::error::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ProviderDisbursementType {
    Deferred,
    #[default]
    Instant,
}

impl ProviderDisbursementType {
    pub const fn code(self) -> i32 {
        match self {
            ProviderDisbursementType::Deferred => 1,
            ProviderDisbursementType::Instant => 2,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerateQrisRequest {
    pub username: String,
    pub amount: i64,
    pub expire_seconds: i64,
    pub custom_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedQris {
    pub provider_trx_id: String,
    pub qris_payload: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckPaymentStatusRequest {
    pub provider_trx_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderPaymentStatus {
    Pending,
    Success,
    Failed,
    Expired,
    Unknown(String),
}

impl ProviderPaymentStatus {
    pub fn from_provider_value(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "pending" => Self::Pending,
            "success" => Self::Success,
            "failed" => Self::Failed,
            "expired" => Self::Expired,
            other => Self::Unknown(other.to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedPaymentStatus {
    pub amount: i64,
    pub provider_merchant_id: String,
    pub provider_trx_id: String,
    pub provider_rrn: Option<String>,
    pub status: ProviderPaymentStatus,
    pub provider_created_at: Option<DateTime<Utc>>,
    pub provider_finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InquiryBankRequest {
    pub amount: i64,
    pub bank_code: String,
    pub account_number: String,
    pub disbursement_type: ProviderDisbursementType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InquiryBankResult {
    pub account_number: String,
    pub account_name: String,
    pub bank_code: String,
    pub bank_name: String,
    pub partner_ref_no: String,
    pub vendor_ref_no: Option<String>,
    pub amount: i64,
    pub fee: i64,
    pub inquiry_id: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferRequest {
    pub amount: i64,
    pub bank_code: String,
    pub account_number: String,
    pub disbursement_type: ProviderDisbursementType,
    pub inquiry_id: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferResult {
    pub accepted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckDisbursementStatusRequest {
    pub partner_ref_no: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderDisbursementStatus {
    Pending,
    Processing,
    Success,
    Failed,
    Cancelled,
    Unknown(String),
}

impl ProviderDisbursementStatus {
    pub fn from_provider_value(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "pending" => Self::Pending,
            "processing" => Self::Processing,
            "success" => Self::Success,
            "failed" => Self::Failed,
            "cancelled" => Self::Cancelled,
            other => Self::Unknown(other.to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedDisbursementStatus {
    pub amount: i64,
    pub fee: i64,
    pub partner_ref_no: String,
    pub provider_merchant_id: String,
    pub status: ProviderDisbursementStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GetBalanceRequest;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderBalanceSnapshot {
    pub provider_pending_balance: i64,
    pub provider_settle_balance: i64,
}

#[async_trait]
pub trait PaymentProviderGateway: Send + Sync {
    async fn generate_qris(&self, request: GenerateQrisRequest) -> Result<GeneratedQris, AppError>;

    async fn check_payment_status(
        &self,
        request: CheckPaymentStatusRequest,
    ) -> Result<CheckedPaymentStatus, AppError>;

    async fn inquiry_bank(
        &self,
        request: InquiryBankRequest,
    ) -> Result<InquiryBankResult, AppError>;

    async fn transfer(&self, request: TransferRequest) -> Result<TransferResult, AppError>;

    async fn check_disbursement_status(
        &self,
        request: CheckDisbursementStatusRequest,
    ) -> Result<CheckedDisbursementStatus, AppError>;

    async fn get_balance(
        &self,
        request: GetBalanceRequest,
    ) -> Result<ProviderBalanceSnapshot, AppError>;
}
