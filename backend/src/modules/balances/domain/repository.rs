use async_trait::async_trait;
use uuid::Uuid;

use crate::modules::balances::domain::entity::{
    BalanceSummaryDelta, NewStoreBalanceLedgerEntry, StoreBalanceLedgerEntry,
    StoreBalanceSnapshot, StoreBalanceSummary,
};

#[async_trait]
pub trait StoreBalanceRepository: Send + Sync {
    async fn fetch_store_balance_snapshot(
        &self,
        store_id: Uuid,
        user_scope: Option<Uuid>,
        global_access: bool,
    ) -> anyhow::Result<Option<StoreBalanceSnapshot>>;

    async fn fetch_store_balance_summary(
        &self,
        store_id: Uuid,
    ) -> anyhow::Result<Option<StoreBalanceSummary>>;

    async fn apply_summary_delta(
        &self,
        store_id: Uuid,
        delta: BalanceSummaryDelta,
        updated_at: chrono::DateTime<chrono::Utc>,
    ) -> anyhow::Result<StoreBalanceSummary>;

    async fn insert_ledger_entry(
        &self,
        entry: NewStoreBalanceLedgerEntry,
    ) -> anyhow::Result<StoreBalanceLedgerEntry>;
}
