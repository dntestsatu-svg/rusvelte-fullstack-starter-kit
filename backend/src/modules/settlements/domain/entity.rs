use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::modules::balances::domain::entity::StoreBalanceSnapshot;

#[derive(Debug, Clone, Deserialize)]
pub struct SettlementRequest {
    pub store_id: Uuid,
    pub amount: i64,
    pub notes: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ProcessSettlementCommand {
    pub store_id: Uuid,
    pub amount: i64,
    pub notes: Option<String>,
    pub processed_by_user_id: Uuid,
    pub processed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SettlementStatus {
    Processed,
}

impl SettlementStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Processed => "processed",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SettlementRecord {
    pub id: Uuid,
    pub store_id: Uuid,
    pub amount: i64,
    pub status: SettlementStatus,
    pub processed_by_user_id: Uuid,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessedSettlement {
    pub settlement: SettlementRecord,
    pub balance: StoreBalanceSnapshot,
    pub notification_user_ids: Vec<Uuid>,
}
