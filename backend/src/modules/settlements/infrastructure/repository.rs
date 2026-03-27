use anyhow::anyhow;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

use crate::infrastructure::db::DbPool;
use crate::modules::balances::domain::entity::{
    BalanceBucket, LedgerDirection, StoreBalanceEntryType, StoreBalanceSnapshot,
};
use crate::modules::settlements::domain::entity::{
    ProcessSettlementCommand, ProcessedSettlement, SettlementRecord, SettlementStatus,
};
use crate::modules::settlements::domain::repository::{
    SettlementProcessError, SettlementRepository,
};

pub struct SqlxSettlementRepository {
    db: DbPool,
}

impl SqlxSettlementRepository {
    pub fn new(db: DbPool) -> Self {
        Self { db }
    }
}

#[async_trait]
impl SettlementRepository for SqlxSettlementRepository {
    async fn process_settlement(
        &self,
        command: ProcessSettlementCommand,
    ) -> Result<ProcessedSettlement, SettlementProcessError> {
        let mut transaction = self
            .db
            .begin()
            .await
            .map_err(|error| SettlementProcessError::Unexpected(anyhow!(error)))?;

        let store = sqlx::query_as::<_, StoreRow>(
            r#"
            SELECT owner_user_id, name
            FROM stores
            WHERE id = $1
              AND deleted_at IS NULL
            "#,
        )
        .bind(command.store_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| SettlementProcessError::Unexpected(anyhow!(error)))?
        .ok_or(SettlementProcessError::StoreNotFound(command.store_id))?;

        sqlx::query(
            r#"
            INSERT INTO store_balance_summaries (
                store_id,
                pending_balance,
                settled_balance,
                reserved_settled_balance,
                updated_at
            )
            VALUES ($1, 0, 0, 0, $2)
            ON CONFLICT (store_id) DO NOTHING
            "#,
        )
        .bind(command.store_id)
        .bind(command.processed_at)
        .execute(&mut *transaction)
        .await
        .map_err(|error| SettlementProcessError::Unexpected(anyhow!(error)))?;

        let updated_summary = sqlx::query_as::<_, StoreBalanceSnapshotRow>(
            r#"
            UPDATE store_balance_summaries
            SET pending_balance = pending_balance - $2,
                settled_balance = settled_balance + $2,
                updated_at = $3
            WHERE store_id = $1
              AND pending_balance >= $2
            RETURNING
                store_id,
                pending_balance,
                settled_balance,
                reserved_settled_balance,
                updated_at
            "#,
        )
        .bind(command.store_id)
        .bind(command.amount)
        .bind(command.processed_at)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| SettlementProcessError::Unexpected(anyhow!(error)))?;

        let updated_summary = match updated_summary {
            Some(summary) => summary,
            None => {
                let available = sqlx::query_scalar::<_, i64>(
                    r#"
                    SELECT pending_balance
                    FROM store_balance_summaries
                    WHERE store_id = $1
                    "#,
                )
                .bind(command.store_id)
                .fetch_one(&mut *transaction)
                .await
                .map_err(|error| SettlementProcessError::Unexpected(anyhow!(error)))?;

                return Err(SettlementProcessError::InsufficientPendingBalance {
                    available,
                    requested: command.amount,
                });
            }
        };

        let settlement = sqlx::query_as::<_, SettlementRow>(
            r#"
            INSERT INTO store_balance_settlements (
                store_id,
                amount,
                status,
                processed_by_user_id,
                notes,
                created_at
            )
            VALUES ($1, $2, 'processed', $3, $4, $5)
            RETURNING id, store_id, amount, status, processed_by_user_id, notes, created_at
            "#,
        )
        .bind(command.store_id)
        .bind(command.amount)
        .bind(command.processed_by_user_id)
        .bind(command.notes.clone())
        .bind(command.processed_at)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|error| SettlementProcessError::Unexpected(anyhow!(error)))?;

        let description = command
            .notes
            .clone()
            .unwrap_or_else(|| format!("Manual settlement processed for {}", store.name));

        insert_ledger_entry(
            &mut transaction,
            LedgerEntryInsert {
                store_id: command.store_id,
                related_type: "settlement",
                related_id: Some(settlement.id),
                entry_type: StoreBalanceEntryType::SettlementMovePendingToSettled.as_str(),
                amount: command.amount,
                direction: LedgerDirection::Debit.as_str(),
                balance_bucket: BalanceBucket::Pending.as_str(),
                description: Some(format!("Pending balance moved to settled. {description}")),
                created_at: command.processed_at,
            },
        )
        .await?;

        insert_ledger_entry(
            &mut transaction,
            LedgerEntryInsert {
                store_id: command.store_id,
                related_type: "settlement",
                related_id: Some(settlement.id),
                entry_type: StoreBalanceEntryType::SettlementMovePendingToSettled.as_str(),
                amount: command.amount,
                direction: LedgerDirection::Credit.as_str(),
                balance_bucket: BalanceBucket::Settled.as_str(),
                description: Some(format!("Settled balance increased from pending. {description}")),
                created_at: command.processed_at,
            },
        )
        .await?;

        let notification_user_ids = owner_user_ids(&mut transaction, command.store_id, store.owner_user_id)
            .await?;
        if !notification_user_ids.is_empty() {
            let title = format!("Settlement processed for {}", store.name);
            let body = format!(
                "Developer processed a settlement of Rp {} from pending to settled balance.",
                format_rupiah(command.amount)
            );
            sqlx::query(
                r#"
                INSERT INTO user_notifications (
                    id,
                    user_id,
                    type,
                    title,
                    body,
                    related_type,
                    related_id,
                    status,
                    created_at
                )
                SELECT
                    gen_random_uuid(),
                    owner_user_id,
                    'settlement_processed',
                    $2,
                    $3,
                    'settlement',
                    $4,
                    'unread',
                    $5
                FROM UNNEST($1::uuid[]) AS owner_user_id
                "#,
            )
            .bind(&notification_user_ids)
            .bind(title)
            .bind(body)
            .bind(settlement.id)
            .bind(command.processed_at)
            .execute(&mut *transaction)
            .await
            .map_err(|error| SettlementProcessError::Unexpected(anyhow!(error)))?;
        }

        sqlx::query(
            r#"
            INSERT INTO audit_logs (
                actor_user_id,
                action,
                target_type,
                target_id,
                payload_json,
                created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(command.processed_by_user_id)
        .bind("settlement.processed")
        .bind("store_balance_settlement")
        .bind(settlement.id)
        .bind(serde_json::json!({
            "store_id": command.store_id,
            "amount": command.amount,
            "notes": command.notes,
            "pending_balance": updated_summary.pending_balance,
            "settled_balance": updated_summary.settled_balance,
        }))
        .bind(command.processed_at)
        .execute(&mut *transaction)
        .await
        .map_err(|error| SettlementProcessError::Unexpected(anyhow!(error)))?;

        transaction
            .commit()
            .await
            .map_err(|error| SettlementProcessError::Unexpected(anyhow!(error)))?;

        Ok(ProcessedSettlement {
            settlement: settlement.into(),
            balance: updated_summary.into(),
            notification_user_ids,
        })
    }
}

async fn owner_user_ids(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    store_id: Uuid,
    owner_user_id: Uuid,
) -> Result<Vec<Uuid>, SettlementProcessError> {
    let mut ids = sqlx::query_scalar::<_, Uuid>(
        r#"
        SELECT DISTINCT user_id
        FROM store_members
        WHERE store_id = $1
          AND store_role = 'owner'
          AND status = 'active'
        "#,
    )
    .bind(store_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| SettlementProcessError::Unexpected(anyhow!(error)))?;

    if !ids.iter().any(|value| *value == owner_user_id) {
        ids.push(owner_user_id);
    }

    Ok(ids)
}

async fn insert_ledger_entry(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    entry: LedgerEntryInsert<'_>,
) -> Result<(), SettlementProcessError> {
    sqlx::query(
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
        "#,
    )
    .bind(entry.store_id)
    .bind(entry.related_type)
    .bind(entry.related_id)
    .bind(entry.entry_type)
    .bind(entry.amount)
    .bind(entry.direction)
    .bind(entry.balance_bucket)
    .bind(entry.description)
    .bind(entry.created_at)
    .execute(&mut **transaction)
    .await
    .map_err(|error| SettlementProcessError::Unexpected(anyhow!(error)))?;

    Ok(())
}

fn format_rupiah(amount: i64) -> String {
    let digits = amount.abs().to_string();
    let mut groups = Vec::new();
    let bytes = digits.as_bytes();

    for chunk in bytes.rchunks(3) {
        groups.push(std::str::from_utf8(chunk).unwrap_or_default().to_string());
    }

    groups.reverse();
    let joined = groups.join(".");
    if amount < 0 {
        format!("-{joined}")
    } else {
        joined
    }
}

struct LedgerEntryInsert<'a> {
    store_id: Uuid,
    related_type: &'a str,
    related_id: Option<Uuid>,
    entry_type: &'a str,
    amount: i64,
    direction: &'a str,
    balance_bucket: &'a str,
    description: Option<String>,
    created_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct StoreRow {
    owner_user_id: Uuid,
    name: String,
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
struct SettlementRow {
    id: Uuid,
    store_id: Uuid,
    amount: i64,
    status: String,
    processed_by_user_id: Uuid,
    notes: Option<String>,
    created_at: DateTime<Utc>,
}

impl From<StoreBalanceSnapshotRow> for StoreBalanceSnapshot {
    fn from(value: StoreBalanceSnapshotRow) -> Self {
        Self {
            store_id: value.store_id,
            pending_balance: value.pending_balance,
            settled_balance: value.settled_balance,
            reserved_settled_balance: value.reserved_settled_balance,
            withdrawable_balance: value.settled_balance - value.reserved_settled_balance,
            updated_at: value.updated_at,
        }
    }
}

impl From<SettlementRow> for SettlementRecord {
    fn from(value: SettlementRow) -> Self {
        Self {
            id: value.id,
            store_id: value.store_id,
            amount: value.amount,
            status: match value.status.as_str() {
                "processed" => SettlementStatus::Processed,
                _ => SettlementStatus::Processed,
            },
            processed_by_user_id: value.processed_by_user_id,
            notes: value.notes,
            created_at: value.created_at,
        }
    }
}
