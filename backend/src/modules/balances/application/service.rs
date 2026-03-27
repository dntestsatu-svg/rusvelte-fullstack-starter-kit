use std::sync::Arc;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::modules::balances::domain::entity::{
    BalanceSummaryDelta, NewStoreBalanceLedgerEntry, StoreBalanceLedgerEntry,
    StoreBalanceSnapshot, StoreBalanceSummary,
};
use crate::modules::balances::domain::repository::StoreBalanceRepository;
use crate::shared::auth::{has_capability, AuthenticatedUser, Capability};
use crate::shared::error::AppError;

pub struct StoreBalanceService {
    repository: Arc<dyn StoreBalanceRepository>,
}

impl StoreBalanceService {
    pub fn new(repository: Arc<dyn StoreBalanceRepository>) -> Self {
        Self { repository }
    }

    pub async fn get_store_balance_snapshot(
        &self,
        store_id: Uuid,
        actor: &AuthenticatedUser,
    ) -> Result<Option<StoreBalanceSnapshot>, AppError> {
        if !has_capability(actor, Capability::BalanceRead, Some(store_id)) {
            return Ok(None);
        }

        let (user_scope, global_access) = balance_scope(actor);
        let snapshot = self
            .repository
            .fetch_store_balance_snapshot(store_id, user_scope, global_access)
            .await?;

        Ok(snapshot)
    }

    pub async fn apply_summary_delta(
        &self,
        store_id: Uuid,
        delta: BalanceSummaryDelta,
        updated_at: DateTime<Utc>,
    ) -> Result<StoreBalanceSummary, AppError> {
        self.repository
            .apply_summary_delta(store_id, delta, updated_at)
            .await
            .map_err(Into::into)
    }

    pub async fn append_ledger_entry(
        &self,
        entry: NewStoreBalanceLedgerEntry,
    ) -> Result<StoreBalanceLedgerEntry, AppError> {
        self.repository
            .insert_ledger_entry(entry)
            .await
            .map_err(Into::into)
    }
}

fn balance_scope(actor: &AuthenticatedUser) -> (Option<Uuid>, bool) {
    if has_capability(actor, Capability::BalanceReadGlobal, None) {
        (Some(actor.user_id), true)
    } else {
        (Some(actor.user_id), false)
    }
}
