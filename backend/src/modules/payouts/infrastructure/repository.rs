use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use crate::modules::payouts::domain::entity::{
    NewPayoutRecord, PayoutListRow, PayoutRecord, PayoutStatus, UpdatePayoutStatus,
};
use crate::modules::payouts::domain::repository::PayoutRepository;

pub struct SqlxPayoutRepository {
    pool: PgPool,
}

impl SqlxPayoutRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PayoutRepository for SqlxPayoutRepository {
    async fn insert_payout(&self, record: NewPayoutRecord) -> anyhow::Result<PayoutRecord> {
        let id = Uuid::new_v4();
        let status_str = record.status.as_str();
        let now = record.created_at;

        sqlx::query!(
            r#"
            INSERT INTO store_payout_requests (
                id, store_id, bank_account_id, requested_by_user_id,
                requested_amount, platform_withdraw_fee_bps, platform_withdraw_fee_amount,
                provider_withdraw_fee_amount, net_disbursed_amount,
                provider_partner_ref_no, provider_inquiry_id,
                status, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            "#,
            id,
            record.store_id,
            record.bank_account_id,
            record.requested_by_user_id,
            record.requested_amount,
            record.platform_withdraw_fee_bps,
            record.platform_withdraw_fee_amount,
            record.provider_withdraw_fee_amount,
            record.net_disbursed_amount,
            record.provider_partner_ref_no,
            record.provider_inquiry_id,
            status_str,
            now,
            now,
        )
        .execute(&self.pool)
        .await?;

        Ok(PayoutRecord {
            id,
            store_id: record.store_id,
            bank_account_id: record.bank_account_id,
            requested_by_user_id: record.requested_by_user_id,
            requested_amount: record.requested_amount,
            platform_withdraw_fee_bps: record.platform_withdraw_fee_bps,
            platform_withdraw_fee_amount: record.platform_withdraw_fee_amount,
            provider_withdraw_fee_amount: record.provider_withdraw_fee_amount,
            net_disbursed_amount: record.net_disbursed_amount,
            provider_partner_ref_no: record.provider_partner_ref_no,
            provider_inquiry_id: record.provider_inquiry_id,
            status: record.status,
            failure_reason: None,
            provider_transaction_date: None,
            processed_at: None,
            created_at: now,
            updated_at: now,
        })
    }

    async fn update_status(
        &self,
        update: UpdatePayoutStatus,
    ) -> anyhow::Result<Option<PayoutRecord>> {
        let status_str = update.new_status.as_str();

        let row = sqlx::query!(
            r#"
            UPDATE store_payout_requests
            SET status = $1, failure_reason = $2, updated_at = $3
            WHERE id = $4 AND store_id = $5
            RETURNING id, store_id, bank_account_id, requested_by_user_id,
                requested_amount, platform_withdraw_fee_bps, platform_withdraw_fee_amount,
                provider_withdraw_fee_amount, net_disbursed_amount,
                provider_partner_ref_no, provider_inquiry_id,
                status, failure_reason, provider_transaction_date,
                processed_at, created_at, updated_at
            "#,
            status_str,
            update.failure_reason,
            update.updated_at,
            update.payout_id,
            update.store_id,
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| PayoutRecord {
            id: r.id,
            store_id: r.store_id,
            bank_account_id: r.bank_account_id,
            requested_by_user_id: r.requested_by_user_id,
            requested_amount: r.requested_amount,
            platform_withdraw_fee_bps: r.platform_withdraw_fee_bps,
            platform_withdraw_fee_amount: r.platform_withdraw_fee_amount,
            provider_withdraw_fee_amount: r.provider_withdraw_fee_amount,
            net_disbursed_amount: r.net_disbursed_amount,
            provider_partner_ref_no: r.provider_partner_ref_no,
            provider_inquiry_id: r.provider_inquiry_id,
            status: PayoutStatus::from_db(&r.status),
            failure_reason: r.failure_reason,
            provider_transaction_date: r.provider_transaction_date,
            processed_at: r.processed_at,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }))
    }

    async fn find_by_id(
        &self,
        store_id: Uuid,
        payout_id: Uuid,
    ) -> anyhow::Result<Option<PayoutRecord>> {
        let row = sqlx::query!(
            r#"
            SELECT id, store_id, bank_account_id, requested_by_user_id,
                requested_amount, platform_withdraw_fee_bps, platform_withdraw_fee_amount,
                provider_withdraw_fee_amount, net_disbursed_amount,
                provider_partner_ref_no, provider_inquiry_id,
                status, failure_reason, provider_transaction_date,
                processed_at, created_at, updated_at
            FROM store_payout_requests
            WHERE id = $1 AND store_id = $2
            "#,
            payout_id,
            store_id,
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| PayoutRecord {
            id: r.id,
            store_id: r.store_id,
            bank_account_id: r.bank_account_id,
            requested_by_user_id: r.requested_by_user_id,
            requested_amount: r.requested_amount,
            platform_withdraw_fee_bps: r.platform_withdraw_fee_bps,
            platform_withdraw_fee_amount: r.platform_withdraw_fee_amount,
            provider_withdraw_fee_amount: r.provider_withdraw_fee_amount,
            net_disbursed_amount: r.net_disbursed_amount,
            provider_partner_ref_no: r.provider_partner_ref_no,
            provider_inquiry_id: r.provider_inquiry_id,
            status: PayoutStatus::from_db(&r.status),
            failure_reason: r.failure_reason,
            provider_transaction_date: r.provider_transaction_date,
            processed_at: r.processed_at,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }))
    }

    async fn list_by_store(
        &self,
        store_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> anyhow::Result<Vec<PayoutListRow>> {
        let rows = sqlx::query!(
            r#"
            SELECT
                p.id,
                p.store_id,
                p.requested_amount,
                p.platform_withdraw_fee_amount,
                p.provider_withdraw_fee_amount,
                p.net_disbursed_amount,
                p.status,
                b.bank_name,
                b.account_number_last4,
                b.account_holder_name,
                p.created_at
            FROM store_payout_requests p
            JOIN store_bank_accounts b ON b.id = p.bank_account_id
            WHERE p.store_id = $1
            ORDER BY p.created_at DESC
            LIMIT $2 OFFSET $3
            "#,
            store_id,
            limit,
            offset,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| PayoutListRow {
                id: r.id,
                store_id: r.store_id,
                requested_amount: r.requested_amount,
                platform_withdraw_fee_amount: r.platform_withdraw_fee_amount,
                provider_withdraw_fee_amount: r.provider_withdraw_fee_amount,
                net_disbursed_amount: r.net_disbursed_amount,
                status: PayoutStatus::from_db(&r.status),
                bank_name: r.bank_name,
                account_number_last4: r.account_number_last4,
                account_holder_name: r.account_holder_name,
                created_at: r.created_at,
            })
            .collect())
    }
}
