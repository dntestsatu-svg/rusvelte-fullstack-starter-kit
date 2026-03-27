use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

use crate::infrastructure::db::DbPool;
use crate::modules::balances::domain::entity::{
    BalanceBucket, BalanceSummaryDelta, LedgerDirection, NewStoreBalanceLedgerEntry,
    StoreBalanceEntryType, StoreBalanceLedgerEntry, StoreBalanceSnapshot, StoreBalanceSummary,
};
use crate::modules::balances::domain::repository::StoreBalanceRepository;

pub struct SqlxStoreBalanceRepository {
    db: DbPool,
}

impl SqlxStoreBalanceRepository {
    pub fn new(db: DbPool) -> Self {
        Self { db }
    }
}

#[async_trait]
impl StoreBalanceRepository for SqlxStoreBalanceRepository {
    async fn fetch_store_balance_snapshot(
        &self,
        store_id: Uuid,
        user_scope: Option<Uuid>,
        global_access: bool,
    ) -> anyhow::Result<Option<StoreBalanceSnapshot>> {
        let row = sqlx::query_as::<_, StoreBalanceSnapshotRow>(
            r#"
            SELECT
                s.id AS store_id,
                COALESCE(b.pending_balance, 0) AS pending_balance,
                COALESCE(b.settled_balance, 0) AS settled_balance,
                COALESCE(b.reserved_settled_balance, 0) AS reserved_settled_balance,
                COALESCE(b.updated_at, s.created_at) AS updated_at
            FROM stores s
            LEFT JOIN store_balance_summaries b ON b.store_id = s.id
            WHERE s.id = $1
              AND s.deleted_at IS NULL
              AND (
                $2::BOOLEAN = TRUE
                OR EXISTS (
                    SELECT 1
                    FROM store_members sm
                    WHERE sm.store_id = s.id
                      AND sm.user_id = $3
                      AND sm.status = 'active'
                )
              )
            "#,
        )
        .bind(store_id)
        .bind(global_access)
        .bind(user_scope)
        .fetch_optional(&self.db)
        .await?;

        Ok(row.map(|record| StoreBalanceSnapshot {
            store_id: record.store_id,
            pending_balance: record.pending_balance,
            settled_balance: record.settled_balance,
            reserved_settled_balance: record.reserved_settled_balance,
            withdrawable_balance: record.settled_balance - record.reserved_settled_balance,
            updated_at: record.updated_at,
        }))
    }

    async fn fetch_store_balance_summary(
        &self,
        store_id: Uuid,
    ) -> anyhow::Result<Option<StoreBalanceSummary>> {
        let row = sqlx::query_as::<_, StoreBalanceSummaryRow>(
            r#"
            SELECT store_id, pending_balance, settled_balance, reserved_settled_balance, updated_at
            FROM store_balance_summaries
            WHERE store_id = $1
            "#,
        )
        .bind(store_id)
        .fetch_optional(&self.db)
        .await?;

        Ok(row.map(Into::into))
    }

    async fn apply_summary_delta(
        &self,
        store_id: Uuid,
        delta: BalanceSummaryDelta,
        updated_at: DateTime<Utc>,
    ) -> anyhow::Result<StoreBalanceSummary> {
        let row = sqlx::query_as::<_, StoreBalanceSummaryRow>(
            r#"
            INSERT INTO store_balance_summaries (
                store_id,
                pending_balance,
                settled_balance,
                reserved_settled_balance,
                updated_at
            )
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (store_id)
            DO UPDATE SET
                pending_balance = store_balance_summaries.pending_balance + EXCLUDED.pending_balance,
                settled_balance = store_balance_summaries.settled_balance + EXCLUDED.settled_balance,
                reserved_settled_balance = store_balance_summaries.reserved_settled_balance + EXCLUDED.reserved_settled_balance,
                updated_at = EXCLUDED.updated_at
            RETURNING store_id, pending_balance, settled_balance, reserved_settled_balance, updated_at
            "#,
        )
        .bind(store_id)
        .bind(delta.pending_delta)
        .bind(delta.settled_delta)
        .bind(delta.reserved_delta)
        .bind(updated_at)
        .fetch_one(&self.db)
        .await?;

        Ok(row.into())
    }

    async fn insert_ledger_entry(
        &self,
        entry: NewStoreBalanceLedgerEntry,
    ) -> anyhow::Result<StoreBalanceLedgerEntry> {
        let row = sqlx::query_as::<_, StoreBalanceLedgerEntryRow>(
            r#"
            INSERT INTO store_balance_ledger_entries (
                store_id,
                related_type,
                related_id,
                entry_type,
                amount,
                direction,
                balance_bucket,
                description,
                created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING
                id,
                store_id,
                related_type,
                related_id,
                entry_type,
                amount,
                direction,
                balance_bucket,
                description,
                created_at
            "#,
        )
        .bind(entry.store_id)
        .bind(entry.related_type)
        .bind(entry.related_id)
        .bind(entry.entry_type.as_str())
        .bind(entry.amount)
        .bind(entry.direction.as_str())
        .bind(entry.balance_bucket.as_str())
        .bind(entry.description)
        .bind(entry.created_at)
        .fetch_one(&self.db)
        .await?;

        Ok(row.into())
    }
}

#[derive(Debug, FromRow)]
struct StoreBalanceSnapshotRow {
    store_id: Uuid,
    pending_balance: i64,
    settled_balance: i64,
    reserved_settled_balance: i64,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct StoreBalanceSummaryRow {
    store_id: Uuid,
    pending_balance: i64,
    settled_balance: i64,
    reserved_settled_balance: i64,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct StoreBalanceLedgerEntryRow {
    id: Uuid,
    store_id: Uuid,
    related_type: String,
    related_id: Option<Uuid>,
    entry_type: String,
    amount: i64,
    direction: String,
    balance_bucket: String,
    description: Option<String>,
    created_at: DateTime<Utc>,
}

impl From<StoreBalanceSummaryRow> for StoreBalanceSummary {
    fn from(value: StoreBalanceSummaryRow) -> Self {
        Self {
            store_id: value.store_id,
            pending_balance: value.pending_balance,
            settled_balance: value.settled_balance,
            reserved_settled_balance: value.reserved_settled_balance,
            updated_at: value.updated_at,
        }
    }
}

impl From<StoreBalanceLedgerEntryRow> for StoreBalanceLedgerEntry {
    fn from(value: StoreBalanceLedgerEntryRow) -> Self {
        Self {
            id: value.id,
            store_id: value.store_id,
            related_type: value.related_type,
            related_id: value.related_id,
            entry_type: match value.entry_type.as_str() {
                "payment_success_credit_pending" => {
                    StoreBalanceEntryType::PaymentSuccessCreditPending
                }
                "settlement_move_pending_to_settled" => {
                    StoreBalanceEntryType::SettlementMovePendingToSettled
                }
                "payout_reserve_settled" => StoreBalanceEntryType::PayoutReserveSettled,
                "payout_success_debit_settled" => StoreBalanceEntryType::PayoutSuccessDebitSettled,
                "payout_failed_release_reserve" => {
                    StoreBalanceEntryType::PayoutFailedReleaseReserve
                }
                _ => StoreBalanceEntryType::ManualAdjustment,
            },
            amount: value.amount,
            direction: match value.direction.as_str() {
                "debit" => LedgerDirection::Debit,
                _ => LedgerDirection::Credit,
            },
            balance_bucket: match value.balance_bucket.as_str() {
                "settled" => BalanceBucket::Settled,
                "reserved" => BalanceBucket::Reserved,
                _ => BalanceBucket::Pending,
            },
            description: value.description,
            created_at: value.created_at,
        }
    }
}
