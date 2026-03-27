use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PayoutStatus {
    Previewed,
    PendingProvider,
    Processing,
    Success,
    Failed,
    Cancelled,
}

impl PayoutStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Previewed => "previewed",
            Self::PendingProvider => "pending_provider",
            Self::Processing => "processing",
            Self::Success => "success",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "previewed" => Self::Previewed,
            "pending_provider" => Self::PendingProvider,
            "processing" => Self::Processing,
            "success" => Self::Success,
            "failed" => Self::Failed,
            "cancelled" => Self::Cancelled,
            _ => Self::Failed,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayoutRecord {
    pub id: Uuid,
    pub store_id: Uuid,
    pub bank_account_id: Uuid,
    pub requested_by_user_id: Uuid,
    pub requested_amount: i64,
    pub platform_withdraw_fee_bps: i32,
    pub platform_withdraw_fee_amount: i64,
    pub provider_withdraw_fee_amount: i64,
    pub net_disbursed_amount: i64,
    pub provider_partner_ref_no: Option<String>,
    pub provider_inquiry_id: Option<String>,
    pub status: PayoutStatus,
    pub failure_reason: Option<String>,
    pub provider_transaction_date: Option<DateTime<Utc>>,
    pub processed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayoutListRow {
    pub id: Uuid,
    pub store_id: Uuid,
    pub requested_amount: i64,
    pub platform_withdraw_fee_amount: i64,
    pub provider_withdraw_fee_amount: i64,
    pub net_disbursed_amount: i64,
    pub status: PayoutStatus,
    pub bank_name: String,
    pub account_number_last4: String,
    pub account_holder_name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayoutPreviewResult {
    pub requested_amount: i64,
    pub platform_fee_bps: u32,
    pub platform_fee_amount: i64,
    pub provider_fee_amount: i64,
    pub net_disbursed_amount: i64,
    pub bank_code: String,
    pub bank_name: String,
    pub account_holder_name: String,
    pub account_number_last4: String,
    pub withdrawable_balance: i64,
    pub partner_ref_no: String,
    pub inquiry_id: i64,
}

#[derive(Debug, Clone)]
pub struct NewPayoutRecord {
    pub store_id: Uuid,
    pub bank_account_id: Uuid,
    pub requested_by_user_id: Uuid,
    pub requested_amount: i64,
    pub platform_withdraw_fee_bps: i32,
    pub platform_withdraw_fee_amount: i64,
    pub provider_withdraw_fee_amount: i64,
    pub net_disbursed_amount: i64,
    pub provider_partner_ref_no: Option<String>,
    pub provider_inquiry_id: Option<String>,
    pub status: PayoutStatus,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct UpdatePayoutStatus {
    pub payout_id: Uuid,
    pub store_id: Uuid,
    pub new_status: PayoutStatus,
    pub failure_reason: Option<String>,
    pub updated_at: DateTime<Utc>,
}
