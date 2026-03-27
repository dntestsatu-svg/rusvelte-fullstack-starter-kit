use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoreBalanceSummary {
    pub store_id: Uuid,
    pub pending_balance: i64,
    pub settled_balance: i64,
    pub reserved_settled_balance: i64,
    pub updated_at: DateTime<Utc>,
}

impl StoreBalanceSummary {
    pub fn withdrawable_balance(&self) -> i64 {
        self.settled_balance - self.reserved_settled_balance
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoreBalanceSnapshot {
    pub store_id: Uuid,
    pub pending_balance: i64,
    pub settled_balance: i64,
    pub reserved_settled_balance: i64,
    pub withdrawable_balance: i64,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BalanceBucket {
    Pending,
    Settled,
    Reserved,
}

impl BalanceBucket {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Settled => "settled",
            Self::Reserved => "reserved",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LedgerDirection {
    Credit,
    Debit,
}

impl LedgerDirection {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Credit => "credit",
            Self::Debit => "debit",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StoreBalanceEntryType {
    PaymentSuccessCreditPending,
    SettlementMovePendingToSettled,
    PayoutReserveSettled,
    PayoutSuccessDebitSettled,
    PayoutFailedReleaseReserve,
    ManualAdjustment,
}

impl StoreBalanceEntryType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PaymentSuccessCreditPending => "payment_success_credit_pending",
            Self::SettlementMovePendingToSettled => "settlement_move_pending_to_settled",
            Self::PayoutReserveSettled => "payout_reserve_settled",
            Self::PayoutSuccessDebitSettled => "payout_success_debit_settled",
            Self::PayoutFailedReleaseReserve => "payout_failed_release_reserve",
            Self::ManualAdjustment => "manual_adjustment",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoreBalanceLedgerEntry {
    pub id: Uuid,
    pub store_id: Uuid,
    pub related_type: String,
    pub related_id: Option<Uuid>,
    pub entry_type: StoreBalanceEntryType,
    pub amount: i64,
    pub direction: LedgerDirection,
    pub balance_bucket: BalanceBucket,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewStoreBalanceLedgerEntry {
    pub store_id: Uuid,
    pub related_type: String,
    pub related_id: Option<Uuid>,
    pub entry_type: StoreBalanceEntryType,
    pub amount: i64,
    pub direction: LedgerDirection,
    pub balance_bucket: BalanceBucket,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct BalanceSummaryDelta {
    pub pending_delta: i64,
    pub settled_delta: i64,
    pub reserved_delta: i64,
}
