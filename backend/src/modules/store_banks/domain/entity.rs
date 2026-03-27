use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StoreBankVerificationStatus {
    Verified,
}

impl StoreBankVerificationStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Verified => "verified",
        }
    }
}

impl From<String> for StoreBankVerificationStatus {
    fn from(value: String) -> Self {
        match value.as_str() {
            "verified" => Self::Verified,
            _ => Self::Verified,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoreBankAccountSummary {
    pub id: Uuid,
    pub store_id: Uuid,
    pub owner_user_id: Uuid,
    pub bank_code: String,
    pub bank_name: String,
    pub account_holder_name: String,
    pub account_number_last4: String,
    pub is_default: bool,
    pub verification_status: StoreBankVerificationStatus,
    pub verified_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoreBankInquiry {
    pub bank_code: String,
    pub bank_name: String,
    pub account_holder_name: String,
    pub account_number_last4: String,
    pub provider_fee_amount: i64,
    pub partner_ref_no: String,
    pub vendor_ref_no: Option<String>,
    pub inquiry_id: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreBankAccountSecret {
    pub account: StoreBankAccountSummary,
    pub account_number_plaintext: String,
}

#[derive(Debug, Clone)]
pub struct StoreBankStoreProfile {
    pub store_id: Uuid,
    pub owner_user_id: Uuid,
    pub store_name: String,
}

#[derive(Debug, Clone)]
pub struct NewStoreBankAccountRecord {
    pub id: Uuid,
    pub store_id: Uuid,
    pub owner_user_id: Uuid,
    pub bank_code: String,
    pub bank_name: String,
    pub account_holder_name: String,
    pub account_number_plaintext: String,
    pub account_number_last4: String,
    pub is_default: bool,
    pub verification_status: StoreBankVerificationStatus,
    pub verified_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
