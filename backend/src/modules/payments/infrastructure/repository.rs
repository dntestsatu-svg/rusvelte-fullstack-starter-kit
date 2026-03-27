use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{types::Json, FromRow};
use uuid::Uuid;

use crate::infrastructure::db::DbPool;
use crate::modules::payments::domain::entity::{
    DashboardPaymentDetail, DashboardPaymentSummary, NewPaymentIdempotencyRecord,
    NewPaymentRecord, NewProviderWebhookEventRecord, Payment, PaymentIdempotencyRecord,
    PaymentIdempotencyStatus, PaymentPendingUpdate, PaymentStatus, PaymentWebhookFinalizeCommand,
    PaymentWebhookFinalizeOutcome, PaymentWebhookFinalizeOutcomeKind, PaymentWebhookTarget,
    ProviderWebhookEvent, ProviderWebhookKind, StoreProviderProfile,
};
use crate::modules::payments::domain::repository::{
    PaymentIdempotencyRepository, PaymentRepository,
};

pub struct SqlxPaymentRepository {
    db: DbPool,
}

impl SqlxPaymentRepository {
    pub fn new(db: DbPool) -> Self {
        Self { db }
    }
}

#[async_trait]
impl PaymentRepository for SqlxPaymentRepository {
    async fn find_store_provider_profile(
        &self,
        store_id: Uuid,
    ) -> anyhow::Result<Option<StoreProviderProfile>> {
        let row = sqlx::query_as::<_, StoreProviderProfileRow>(
            r#"
            SELECT id, provider_username
            FROM stores
            WHERE id = $1
              AND deleted_at IS NULL
            "#,
        )
        .bind(store_id)
        .fetch_optional(&self.db)
        .await?;

        Ok(row.map(|record| StoreProviderProfile {
            store_id: record.id,
            provider_username: record.provider_username.unwrap_or_default(),
        }))
    }

    async fn insert_payment(&self, payment: NewPaymentRecord) -> anyhow::Result<Payment> {
        let row = sqlx::query_as::<_, PaymentRow>(
            r#"
            INSERT INTO payments (
                id,
                store_id,
                created_by_user_id,
                provider_name,
                provider_terminal_id,
                merchant_order_id,
                custom_ref,
                gross_amount,
                platform_tx_fee_bps,
                platform_tx_fee_amount,
                store_pending_credit_amount,
                status,
                expired_at,
                created_at,
                updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
            RETURNING
                id,
                store_id,
                created_by_user_id,
                provider_name,
                provider_terminal_id,
                provider_trx_id,
                provider_rrn,
                merchant_order_id,
                custom_ref,
                gross_amount,
                platform_tx_fee_bps,
                platform_tx_fee_amount,
                store_pending_credit_amount,
                status,
                qris_payload,
                expired_at,
                provider_created_at,
                provider_finished_at,
                finalized_at,
                created_at,
                updated_at
            "#,
        )
        .bind(payment.id)
        .bind(payment.store_id)
        .bind(payment.created_by_user_id)
        .bind(payment.provider_name)
        .bind(payment.provider_terminal_id)
        .bind(payment.merchant_order_id)
        .bind(payment.custom_ref)
        .bind(payment.gross_amount)
        .bind(payment.platform_tx_fee_bps)
        .bind(payment.platform_tx_fee_amount)
        .bind(payment.store_pending_credit_amount)
        .bind(payment.status.to_string())
        .bind(payment.expired_at)
        .bind(payment.created_at)
        .bind(payment.updated_at)
        .fetch_one(&self.db)
        .await?;

        Ok(row.into())
    }

    async fn mark_payment_pending(&self, update: PaymentPendingUpdate) -> anyhow::Result<Payment> {
        let row = sqlx::query_as::<_, PaymentRow>(
            r#"
            UPDATE payments
            SET
                provider_trx_id = $2,
                qris_payload = $3,
                provider_created_at = $4,
                status = 'pending',
                updated_at = $5
            WHERE id = $1
            RETURNING
                id,
                store_id,
                created_by_user_id,
                provider_name,
                provider_terminal_id,
                provider_trx_id,
                provider_rrn,
                merchant_order_id,
                custom_ref,
                gross_amount,
                platform_tx_fee_bps,
                platform_tx_fee_amount,
                store_pending_credit_amount,
                status,
                qris_payload,
                expired_at,
                provider_created_at,
                provider_finished_at,
                finalized_at,
                created_at,
                updated_at
            "#,
        )
        .bind(update.payment_id)
        .bind(update.provider_trx_id)
        .bind(update.qris_payload)
        .bind(update.provider_created_at)
        .bind(update.updated_at)
        .fetch_one(&self.db)
        .await?;

        Ok(row.into())
    }

    async fn find_payment_by_id_for_store(
        &self,
        store_id: Uuid,
        payment_id: Uuid,
    ) -> anyhow::Result<Option<Payment>> {
        let row = sqlx::query_as::<_, PaymentRow>(
            r#"
            SELECT
                id,
                store_id,
                created_by_user_id,
                provider_name,
                provider_terminal_id,
                provider_trx_id,
                provider_rrn,
                merchant_order_id,
                custom_ref,
                gross_amount,
                platform_tx_fee_bps,
                platform_tx_fee_amount,
                store_pending_credit_amount,
                status,
                qris_payload,
                expired_at,
                provider_created_at,
                provider_finished_at,
                finalized_at,
                created_at,
                updated_at
            FROM payments
            WHERE id = $1
              AND store_id = $2
            "#,
        )
        .bind(payment_id)
        .bind(store_id)
        .fetch_optional(&self.db)
        .await?;

        Ok(row.map(Into::into))
    }

    async fn list_dashboard_payments(
        &self,
        limit: i64,
        offset: i64,
        search: Option<&str>,
        status: Option<&str>,
        user_scope: Option<Uuid>,
        global_access: bool,
    ) -> anyhow::Result<Vec<DashboardPaymentSummary>> {
        let search = search.map(str::trim).filter(|value| !value.is_empty());
        let rows = sqlx::query_as::<_, DashboardPaymentSummaryRow>(
            r#"
            SELECT
                p.id,
                p.store_id,
                s.name AS store_name,
                s.slug AS store_slug,
                p.gross_amount,
                p.platform_tx_fee_amount,
                p.store_pending_credit_amount,
                p.status,
                p.provider_trx_id,
                p.merchant_order_id,
                p.custom_ref,
                p.expired_at,
                p.finalized_at,
                p.created_at,
                p.updated_at
            FROM payments p
            INNER JOIN stores s ON s.id = p.store_id
            WHERE s.deleted_at IS NULL
              AND (
                $1::BOOLEAN = TRUE
                OR EXISTS (
                    SELECT 1
                    FROM store_members sm
                    WHERE sm.store_id = p.store_id
                      AND sm.user_id = $2
                      AND sm.status = 'active'
                )
              )
              AND ($3::TEXT IS NULL OR p.status = $3)
              AND (
                $4::TEXT IS NULL
                OR s.name ILIKE '%' || $4 || '%'
                OR s.slug ILIKE '%' || $4 || '%'
                OR COALESCE(p.provider_trx_id, '') ILIKE '%' || $4 || '%'
                OR COALESCE(p.merchant_order_id, '') ILIKE '%' || $4 || '%'
                OR COALESCE(p.custom_ref, '') ILIKE '%' || $4 || '%'
              )
            ORDER BY p.created_at DESC
            LIMIT $5 OFFSET $6
            "#,
        )
        .bind(global_access)
        .bind(user_scope)
        .bind(status)
        .bind(search)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.db)
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn count_dashboard_payments(
        &self,
        search: Option<&str>,
        status: Option<&str>,
        user_scope: Option<Uuid>,
        global_access: bool,
    ) -> anyhow::Result<i64> {
        let search = search.map(str::trim).filter(|value| !value.is_empty());
        let count = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(p.id)
            FROM payments p
            INNER JOIN stores s ON s.id = p.store_id
            WHERE s.deleted_at IS NULL
              AND (
                $1::BOOLEAN = TRUE
                OR EXISTS (
                    SELECT 1
                    FROM store_members sm
                    WHERE sm.store_id = p.store_id
                      AND sm.user_id = $2
                      AND sm.status = 'active'
                )
              )
              AND ($3::TEXT IS NULL OR p.status = $3)
              AND (
                $4::TEXT IS NULL
                OR s.name ILIKE '%' || $4 || '%'
                OR s.slug ILIKE '%' || $4 || '%'
                OR COALESCE(p.provider_trx_id, '') ILIKE '%' || $4 || '%'
                OR COALESCE(p.merchant_order_id, '') ILIKE '%' || $4 || '%'
                OR COALESCE(p.custom_ref, '') ILIKE '%' || $4 || '%'
              )
            "#,
        )
        .bind(global_access)
        .bind(user_scope)
        .bind(status)
        .bind(search)
        .fetch_one(&self.db)
        .await?;

        Ok(count)
    }

    async fn find_dashboard_payment_by_id(
        &self,
        payment_id: Uuid,
        user_scope: Option<Uuid>,
        global_access: bool,
    ) -> anyhow::Result<Option<DashboardPaymentDetail>> {
        let row = sqlx::query_as::<_, DashboardPaymentDetailRow>(
            r#"
            SELECT
                p.id,
                p.store_id,
                s.name AS store_name,
                s.slug AS store_slug,
                p.provider_name,
                p.provider_terminal_id,
                p.provider_trx_id,
                p.provider_rrn,
                p.merchant_order_id,
                p.custom_ref,
                p.gross_amount,
                p.platform_tx_fee_bps,
                p.platform_tx_fee_amount,
                p.store_pending_credit_amount,
                p.status,
                p.qris_payload,
                p.expired_at,
                p.provider_created_at,
                p.provider_finished_at,
                p.finalized_at,
                p.created_at,
                p.updated_at
            FROM payments p
            INNER JOIN stores s ON s.id = p.store_id
            WHERE p.id = $1
              AND s.deleted_at IS NULL
              AND (
                $2::BOOLEAN = TRUE
                OR EXISTS (
                    SELECT 1
                    FROM store_members sm
                    WHERE sm.store_id = p.store_id
                      AND sm.user_id = $3
                      AND sm.status = 'active'
                )
              )
            "#,
        )
        .bind(payment_id)
        .bind(global_access)
        .bind(user_scope)
        .fetch_optional(&self.db)
        .await?;

        Ok(row.map(Into::into))
    }

    async fn count_dashboard_payment_distribution(
        &self,
        user_scope: Option<Uuid>,
        global_access: bool,
    ) -> anyhow::Result<crate::modules::payments::domain::entity::DashboardPaymentDistribution> {
        let row = sqlx::query!(
            r#"
            SELECT
                COUNT(*) FILTER (WHERE p.status = 'success') AS success,
                COUNT(*) FILTER (WHERE p.status = 'failed') AS failed,
                COUNT(*) FILTER (WHERE p.status = 'expired') AS expired
            FROM payments p
            INNER JOIN stores s ON s.id = p.store_id
            WHERE s.deleted_at IS NULL
              AND (
                $1::BOOLEAN = TRUE
                OR EXISTS (
                    SELECT 1
                    FROM store_members sm
                    WHERE sm.store_id = p.store_id
                      AND sm.user_id = $2
                      AND sm.status = 'active'
                )
              )
            "#,
            global_access,
            user_scope
        )
        .fetch_one(&self.db)
        .await?;

        Ok(crate::modules::payments::domain::entity::DashboardPaymentDistribution {
            success: row.success.unwrap_or(0),
            failed: row.failed.unwrap_or(0),
            expired: row.expired.unwrap_or(0),
        })
    }

    async fn find_payment_by_provider_trx_id(
        &self,
        provider_name: &str,
        provider_trx_id: &str,
    ) -> anyhow::Result<Option<PaymentWebhookTarget>> {
        let row = sqlx::query_as::<_, PaymentWebhookTargetRow>(
            r#"
            SELECT
                p.id,
                p.store_id,
                p.created_by_user_id,
                p.provider_name,
                p.provider_terminal_id,
                p.provider_trx_id,
                p.provider_rrn,
                p.merchant_order_id,
                p.custom_ref,
                p.gross_amount,
                p.platform_tx_fee_bps,
                p.platform_tx_fee_amount,
                p.store_pending_credit_amount,
                p.status,
                p.qris_payload,
                p.expired_at,
                p.provider_created_at,
                p.provider_finished_at,
                p.finalized_at,
                p.created_at,
                p.updated_at,
                s.name AS store_name,
                s.slug AS store_slug,
                s.callback_url,
                s.callback_secret
            FROM payments p
            INNER JOIN stores s ON s.id = p.store_id
            WHERE p.provider_name = $1
              AND p.provider_trx_id = $2
              AND s.deleted_at IS NULL
            "#,
        )
        .bind(provider_name)
        .bind(provider_trx_id)
        .fetch_optional(&self.db)
        .await?;

        Ok(row.map(Into::into))
    }

    async fn insert_provider_webhook_event(
        &self,
        event: NewProviderWebhookEventRecord,
    ) -> anyhow::Result<ProviderWebhookEvent> {
        let row = sqlx::query_as::<_, ProviderWebhookEventRow>(
            r#"
            INSERT INTO provider_webhook_events (
                id,
                provider_name,
                webhook_kind,
                merchant_id,
                provider_trx_id,
                partner_ref_no,
                payload_json,
                created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING
                id,
                provider_name,
                webhook_kind,
                merchant_id,
                provider_trx_id,
                partner_ref_no,
                payload_json,
                is_verified,
                verification_reason,
                is_processed,
                processing_result,
                processed_at,
                created_at
            "#,
        )
        .bind(event.id)
        .bind(event.provider_name)
        .bind(event.webhook_kind.to_string())
        .bind(event.merchant_id)
        .bind(event.provider_trx_id)
        .bind(event.partner_ref_no)
        .bind(Json(event.payload_json))
        .bind(event.created_at)
        .fetch_one(&self.db)
        .await?;

        Ok(row.into())
    }

    async fn mark_provider_webhook_event_result(
        &self,
        event_id: Uuid,
        is_verified: bool,
        verification_reason: Option<&str>,
        is_processed: bool,
        processing_result: Option<&str>,
        processed_at: Option<DateTime<Utc>>,
    ) -> anyhow::Result<ProviderWebhookEvent> {
        let row = sqlx::query_as::<_, ProviderWebhookEventRow>(
            r#"
            UPDATE provider_webhook_events
            SET
                is_verified = $2,
                verification_reason = $3,
                is_processed = $4,
                processing_result = $5,
                processed_at = $6
            WHERE id = $1
            RETURNING
                id,
                provider_name,
                webhook_kind,
                merchant_id,
                provider_trx_id,
                partner_ref_no,
                payload_json,
                is_verified,
                verification_reason,
                is_processed,
                processing_result,
                processed_at,
                created_at
            "#,
        )
        .bind(event_id)
        .bind(is_verified)
        .bind(verification_reason)
        .bind(is_processed)
        .bind(processing_result)
        .bind(processed_at)
        .fetch_one(&self.db)
        .await?;

        Ok(row.into())
    }

    async fn finalize_payment_from_webhook(
        &self,
        command: PaymentWebhookFinalizeCommand,
    ) -> anyhow::Result<PaymentWebhookFinalizeOutcome> {
        let mut transaction = self.db.begin().await?;

        let current = sqlx::query_as::<_, PaymentRow>(
            r#"
            SELECT
                id,
                store_id,
                created_by_user_id,
                provider_name,
                provider_terminal_id,
                provider_trx_id,
                provider_rrn,
                merchant_order_id,
                custom_ref,
                gross_amount,
                platform_tx_fee_bps,
                platform_tx_fee_amount,
                store_pending_credit_amount,
                status,
                qris_payload,
                expired_at,
                provider_created_at,
                provider_finished_at,
                finalized_at,
                created_at,
                updated_at
            FROM payments
            WHERE id = $1
            FOR UPDATE
            "#,
        )
        .bind(command.payment_id)
        .fetch_optional(&mut *transaction)
        .await?;

        let Some(current_payment) = current.map(Payment::from) else {
            let updated_event = sqlx::query_as::<_, ProviderWebhookEventRow>(
                r#"
                UPDATE provider_webhook_events
                SET
                    is_verified = FALSE,
                    verification_reason = 'Payment not found',
                    is_processed = TRUE,
                    processing_result = 'invalid_payment_reference',
                    processed_at = $2
                WHERE id = $1
                RETURNING
                    id,
                    provider_name,
                    webhook_kind,
                    merchant_id,
                    provider_trx_id,
                    partner_ref_no,
                    payload_json,
                    is_verified,
                    verification_reason,
                    is_processed,
                    processing_result,
                    processed_at,
                    created_at
                "#,
            )
            .bind(command.webhook_event_id)
            .bind(command.processed_at)
            .fetch_one(&mut *transaction)
            .await?;
            let _ = updated_event;

            transaction.commit().await?;
            return Ok(PaymentWebhookFinalizeOutcome {
                kind: PaymentWebhookFinalizeOutcomeKind::Invalid,
                payment: None,
                notification_user_ids: vec![],
                callback_enqueued: false,
            });
        };

        if current_payment.status.is_final() {
            let updated_event = sqlx::query_as::<_, ProviderWebhookEventRow>(
                r#"
                UPDATE provider_webhook_events
                SET
                    is_verified = TRUE,
                    verification_reason = NULL,
                    is_processed = TRUE,
                    processing_result = 'already_finalized',
                    processed_at = $2
                WHERE id = $1
                RETURNING
                    id,
                    provider_name,
                    webhook_kind,
                    merchant_id,
                    provider_trx_id,
                    partner_ref_no,
                    payload_json,
                    is_verified,
                    verification_reason,
                    is_processed,
                    processing_result,
                    processed_at,
                    created_at
                "#,
            )
            .bind(command.webhook_event_id)
            .bind(command.processed_at)
            .fetch_one(&mut *transaction)
            .await?;
            let _ = updated_event;

            transaction.commit().await?;
            return Ok(PaymentWebhookFinalizeOutcome {
                kind: PaymentWebhookFinalizeOutcomeKind::AlreadyFinal,
                payment: Some(current_payment),
                notification_user_ids: vec![],
                callback_enqueued: false,
            });
        }

        let updated = sqlx::query_as::<_, PaymentRow>(
            r#"
            UPDATE payments
            SET
                status = $2,
                provider_rrn = COALESCE($3, provider_rrn),
                provider_finished_at = COALESCE($4, provider_finished_at),
                finalized_at = $5,
                updated_at = $5
            WHERE id = $1
            RETURNING
                id,
                store_id,
                created_by_user_id,
                provider_name,
                provider_terminal_id,
                provider_trx_id,
                provider_rrn,
                merchant_order_id,
                custom_ref,
                gross_amount,
                platform_tx_fee_bps,
                platform_tx_fee_amount,
                store_pending_credit_amount,
                status,
                qris_payload,
                expired_at,
                provider_created_at,
                provider_finished_at,
                finalized_at,
                created_at,
                updated_at
            "#,
        )
        .bind(command.payment_id)
        .bind(command.final_status.to_string())
        .bind(command.provider_rrn.clone())
        .bind(command.provider_finished_at)
        .bind(command.processed_at)
        .fetch_one(&mut *transaction)
        .await?;
        let updated_payment = Payment::from(updated);

        sqlx::query(
            r#"
            INSERT INTO payment_events (
                id,
                payment_id,
                event_type,
                old_status,
                new_status,
                source,
                payload_json,
                created_at
            )
            VALUES ($1, $2, $3, $4, $5, 'provider_webhook', $6, $7)
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(updated_payment.id)
        .bind(format!("payment_{}", command.final_status))
        .bind(current_payment.status.to_string())
        .bind(updated_payment.status.to_string())
        .bind(Json(command.payload_json.clone()))
        .bind(command.processed_at)
        .execute(&mut *transaction)
        .await?;

        if updated_payment.status == PaymentStatus::Success {
            sqlx::query(
                r#"
                INSERT INTO platform_ledger_entries (
                    id,
                    related_type,
                    related_id,
                    entry_type,
                    amount,
                    direction,
                    description,
                    created_at
                )
                VALUES ($1, 'payment', $2, 'payment_platform_fee_income', $3, 'credit', $4, $5)
                ON CONFLICT DO NOTHING
                "#,
            )
            .bind(Uuid::new_v4())
            .bind(updated_payment.id)
            .bind(updated_payment.platform_tx_fee_amount)
            .bind(format!(
                "Platform fee recognized for payment {}",
                updated_payment.id
            ))
            .bind(command.processed_at)
            .execute(&mut *transaction)
            .await?;

            sqlx::query(
                r#"
                INSERT INTO store_balance_ledger_entries (
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
                )
                VALUES ($1, $2, 'payment', $3, 'payment_success_credit_pending', $4, 'credit', 'pending', $5, $6)
                ON CONFLICT DO NOTHING
                "#,
            )
            .bind(Uuid::new_v4())
            .bind(updated_payment.store_id)
            .bind(updated_payment.id)
            .bind(updated_payment.store_pending_credit_amount)
            .bind(format!(
                "Pending balance credit for payment {}",
                updated_payment.id
            ))
            .bind(command.processed_at)
            .execute(&mut *transaction)
            .await?;

            sqlx::query(
                r#"
                INSERT INTO store_balance_summaries (
                    store_id,
                    pending_balance,
                    settled_balance,
                    reserved_settled_balance,
                    updated_at
                )
                VALUES ($1, $2, 0, 0, $3)
                ON CONFLICT (store_id)
                DO UPDATE SET
                    pending_balance = store_balance_summaries.pending_balance + EXCLUDED.pending_balance,
                    updated_at = EXCLUDED.updated_at
                "#,
            )
            .bind(updated_payment.store_id)
            .bind(updated_payment.store_pending_credit_amount)
            .bind(command.processed_at)
            .execute(&mut *transaction)
            .await?;
        }

        let notification_user_ids = sqlx::query_scalar::<_, Uuid>(
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
                recipients.user_id,
                $3,
                $4,
                $5,
                'payment',
                $2,
                'unread',
                $6
            FROM (
                SELECT DISTINCT u.id AS user_id
                FROM users u
                LEFT JOIN store_members sm
                    ON sm.user_id = u.id
                   AND sm.store_id = $1
                   AND sm.status = 'active'
                WHERE u.deleted_at IS NULL
                  AND u.status = 'active'
                  AND (
                    u.role IN ('dev', 'superadmin')
                    OR sm.store_id IS NOT NULL
                  )
            ) recipients
            RETURNING user_id
            "#,
        )
        .bind(updated_payment.store_id)
        .bind(updated_payment.id)
        .bind(command.notification_type.clone())
        .bind(command.notification_title.clone())
        .bind(command.notification_body.clone())
        .bind(command.processed_at)
        .fetch_all(&mut *transaction)
        .await?;

        let callback_enqueued = if let Some(callback_delivery) = &command.callback_delivery {
            sqlx::query(
                r#"
                INSERT INTO callback_deliveries (
                    id,
                    store_id,
                    related_type,
                    related_id,
                    event_type,
                    target_url,
                    signature,
                    status,
                    next_retry_at,
                    final_failure_reason,
                    created_at,
                    updated_at
                )
                VALUES ($1, $2, 'payment', $3, $4, $5, $6, 'queued', NULL, NULL, $7, $7)
                ON CONFLICT DO NOTHING
                "#,
            )
            .bind(Uuid::new_v4())
            .bind(updated_payment.store_id)
            .bind(updated_payment.id)
            .bind(callback_delivery.event_type.clone())
            .bind(callback_delivery.target_url.clone())
            .bind(callback_delivery.signature.clone())
            .bind(command.processed_at)
            .execute(&mut *transaction)
            .await?
            .rows_affected()
                > 0
        } else {
            false
        };

        sqlx::query(
            r#"
            UPDATE provider_webhook_events
            SET
                is_verified = TRUE,
                verification_reason = NULL,
                is_processed = TRUE,
                processing_result = $2,
                processed_at = $3
            WHERE id = $1
            "#,
        )
        .bind(command.webhook_event_id)
        .bind(format!("payment_{}", command.final_status))
        .bind(command.processed_at)
        .execute(&mut *transaction)
        .await?;

        transaction.commit().await?;

        Ok(PaymentWebhookFinalizeOutcome {
            kind: PaymentWebhookFinalizeOutcomeKind::Finalized,
            payment: Some(updated_payment),
            notification_user_ids,
            callback_enqueued,
        })
    }
}

#[async_trait]
impl PaymentIdempotencyRepository for SqlxPaymentRepository {
    async fn find_by_key(
        &self,
        store_id: Uuid,
        idempotency_key: &str,
    ) -> anyhow::Result<Option<PaymentIdempotencyRecord>> {
        let row = sqlx::query_as::<_, PaymentIdempotencyRow>(
            r#"
            SELECT
                id,
                store_id,
                idempotency_key,
                request_hash,
                status,
                response_status_code,
                response_body_json,
                payment_id,
                created_at,
                completed_at,
                updated_at
            FROM payment_idempotency_keys
            WHERE store_id = $1
              AND idempotency_key = $2
            "#,
        )
        .bind(store_id)
        .bind(idempotency_key)
        .fetch_optional(&self.db)
        .await?;

        Ok(row.map(Into::into))
    }

    async fn insert_pending(
        &self,
        record: NewPaymentIdempotencyRecord,
    ) -> anyhow::Result<PaymentIdempotencyRecord> {
        let row = sqlx::query_as::<_, PaymentIdempotencyRow>(
            r#"
            INSERT INTO payment_idempotency_keys (
                id,
                store_id,
                idempotency_key,
                request_hash,
                status,
                created_at,
                updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING
                id,
                store_id,
                idempotency_key,
                request_hash,
                status,
                response_status_code,
                response_body_json,
                payment_id,
                created_at,
                completed_at,
                updated_at
            "#,
        )
        .bind(record.id)
        .bind(record.store_id)
        .bind(record.idempotency_key)
        .bind(record.request_hash)
        .bind(record.status.to_string())
        .bind(record.created_at)
        .bind(record.updated_at)
        .fetch_one(&self.db)
        .await?;

        Ok(row.into())
    }

    async fn complete(
        &self,
        store_id: Uuid,
        idempotency_key: &str,
        response_status_code: i32,
        response_body_json: Value,
        payment_id: Option<Uuid>,
        completed_at: DateTime<Utc>,
    ) -> anyhow::Result<PaymentIdempotencyRecord> {
        let row = sqlx::query_as::<_, PaymentIdempotencyRow>(
            r#"
            UPDATE payment_idempotency_keys
            SET
                status = 'completed',
                response_status_code = $3,
                response_body_json = $4,
                payment_id = $5,
                completed_at = $6,
                updated_at = $6
            WHERE store_id = $1
              AND idempotency_key = $2
            RETURNING
                id,
                store_id,
                idempotency_key,
                request_hash,
                status,
                response_status_code,
                response_body_json,
                payment_id,
                created_at,
                completed_at,
                updated_at
            "#,
        )
        .bind(store_id)
        .bind(idempotency_key)
        .bind(response_status_code)
        .bind(Json(response_body_json))
        .bind(payment_id)
        .bind(completed_at)
        .fetch_one(&self.db)
        .await?;

        Ok(row.into())
    }
}

#[derive(Debug, FromRow)]
struct StoreProviderProfileRow {
    id: Uuid,
    provider_username: Option<String>,
}

#[derive(Debug, FromRow)]
struct PaymentRow {
    id: Uuid,
    store_id: Uuid,
    created_by_user_id: Option<Uuid>,
    provider_name: String,
    provider_terminal_id: Option<String>,
    provider_trx_id: Option<String>,
    provider_rrn: Option<String>,
    merchant_order_id: Option<String>,
    custom_ref: Option<String>,
    gross_amount: i64,
    platform_tx_fee_bps: i32,
    platform_tx_fee_amount: i64,
    store_pending_credit_amount: i64,
    status: String,
    qris_payload: Option<String>,
    expired_at: Option<DateTime<Utc>>,
    provider_created_at: Option<DateTime<Utc>>,
    provider_finished_at: Option<DateTime<Utc>>,
    finalized_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct DashboardPaymentSummaryRow {
    id: Uuid,
    store_id: Uuid,
    store_name: String,
    store_slug: String,
    gross_amount: i64,
    platform_tx_fee_amount: i64,
    store_pending_credit_amount: i64,
    status: String,
    provider_trx_id: Option<String>,
    merchant_order_id: Option<String>,
    custom_ref: Option<String>,
    expired_at: Option<DateTime<Utc>>,
    finalized_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct DashboardPaymentDetailRow {
    id: Uuid,
    store_id: Uuid,
    store_name: String,
    store_slug: String,
    provider_name: String,
    provider_terminal_id: Option<String>,
    provider_trx_id: Option<String>,
    provider_rrn: Option<String>,
    merchant_order_id: Option<String>,
    custom_ref: Option<String>,
    gross_amount: i64,
    platform_tx_fee_bps: i32,
    platform_tx_fee_amount: i64,
    store_pending_credit_amount: i64,
    status: String,
    qris_payload: Option<String>,
    expired_at: Option<DateTime<Utc>>,
    provider_created_at: Option<DateTime<Utc>>,
    provider_finished_at: Option<DateTime<Utc>>,
    finalized_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct PaymentWebhookTargetRow {
    id: Uuid,
    store_id: Uuid,
    created_by_user_id: Option<Uuid>,
    provider_name: String,
    provider_terminal_id: Option<String>,
    provider_trx_id: Option<String>,
    provider_rrn: Option<String>,
    merchant_order_id: Option<String>,
    custom_ref: Option<String>,
    gross_amount: i64,
    platform_tx_fee_bps: i32,
    platform_tx_fee_amount: i64,
    store_pending_credit_amount: i64,
    status: String,
    qris_payload: Option<String>,
    expired_at: Option<DateTime<Utc>>,
    provider_created_at: Option<DateTime<Utc>>,
    provider_finished_at: Option<DateTime<Utc>>,
    finalized_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    store_name: String,
    store_slug: String,
    callback_url: Option<String>,
    callback_secret: Option<String>,
}

#[derive(Debug, FromRow)]
struct ProviderWebhookEventRow {
    id: Uuid,
    provider_name: String,
    webhook_kind: String,
    merchant_id: Option<String>,
    provider_trx_id: Option<String>,
    partner_ref_no: Option<String>,
    payload_json: Json<Value>,
    is_verified: bool,
    verification_reason: Option<String>,
    is_processed: bool,
    processing_result: Option<String>,
    processed_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct PaymentIdempotencyRow {
    id: Uuid,
    store_id: Uuid,
    idempotency_key: String,
    request_hash: String,
    status: String,
    response_status_code: Option<i32>,
    response_body_json: Option<Json<Value>>,
    payment_id: Option<Uuid>,
    created_at: DateTime<Utc>,
    completed_at: Option<DateTime<Utc>>,
    updated_at: DateTime<Utc>,
}

impl From<PaymentRow> for Payment {
    fn from(value: PaymentRow) -> Self {
        Self {
            id: value.id,
            store_id: value.store_id,
            created_by_user_id: value.created_by_user_id,
            provider_name: value.provider_name,
            provider_terminal_id: value.provider_terminal_id,
            provider_trx_id: value.provider_trx_id,
            provider_rrn: value.provider_rrn,
            merchant_order_id: value.merchant_order_id,
            custom_ref: value.custom_ref,
            gross_amount: value.gross_amount,
            platform_tx_fee_bps: value.platform_tx_fee_bps,
            platform_tx_fee_amount: value.platform_tx_fee_amount,
            store_pending_credit_amount: value.store_pending_credit_amount,
            status: PaymentStatus::from(value.status),
            qris_payload: value.qris_payload,
            expired_at: value.expired_at,
            provider_created_at: value.provider_created_at,
            provider_finished_at: value.provider_finished_at,
            finalized_at: value.finalized_at,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

impl From<DashboardPaymentSummaryRow> for DashboardPaymentSummary {
    fn from(value: DashboardPaymentSummaryRow) -> Self {
        Self {
            id: value.id,
            store_id: value.store_id,
            store_name: value.store_name,
            store_slug: value.store_slug,
            gross_amount: value.gross_amount,
            platform_tx_fee_amount: value.platform_tx_fee_amount,
            store_pending_credit_amount: value.store_pending_credit_amount,
            status: PaymentStatus::from(value.status),
            provider_trx_id: value.provider_trx_id,
            merchant_order_id: value.merchant_order_id,
            custom_ref: value.custom_ref,
            expired_at: value.expired_at,
            finalized_at: value.finalized_at,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

impl From<DashboardPaymentDetailRow> for DashboardPaymentDetail {
    fn from(value: DashboardPaymentDetailRow) -> Self {
        Self {
            id: value.id,
            store_id: value.store_id,
            store_name: value.store_name,
            store_slug: value.store_slug,
            provider_name: value.provider_name,
            provider_terminal_id: value.provider_terminal_id,
            provider_trx_id: value.provider_trx_id,
            provider_rrn: value.provider_rrn,
            merchant_order_id: value.merchant_order_id,
            custom_ref: value.custom_ref,
            gross_amount: value.gross_amount,
            platform_tx_fee_bps: value.platform_tx_fee_bps,
            platform_tx_fee_amount: value.platform_tx_fee_amount,
            store_pending_credit_amount: value.store_pending_credit_amount,
            status: PaymentStatus::from(value.status),
            qris_payload: value.qris_payload,
            expired_at: value.expired_at,
            provider_created_at: value.provider_created_at,
            provider_finished_at: value.provider_finished_at,
            finalized_at: value.finalized_at,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

impl From<PaymentWebhookTargetRow> for PaymentWebhookTarget {
    fn from(value: PaymentWebhookTargetRow) -> Self {
        Self {
            payment: Payment {
                id: value.id,
                store_id: value.store_id,
                created_by_user_id: value.created_by_user_id,
                provider_name: value.provider_name,
                provider_terminal_id: value.provider_terminal_id,
                provider_trx_id: value.provider_trx_id,
                provider_rrn: value.provider_rrn,
                merchant_order_id: value.merchant_order_id,
                custom_ref: value.custom_ref,
                gross_amount: value.gross_amount,
                platform_tx_fee_bps: value.platform_tx_fee_bps,
                platform_tx_fee_amount: value.platform_tx_fee_amount,
                store_pending_credit_amount: value.store_pending_credit_amount,
                status: PaymentStatus::from(value.status),
                qris_payload: value.qris_payload,
                expired_at: value.expired_at,
                provider_created_at: value.provider_created_at,
                provider_finished_at: value.provider_finished_at,
                finalized_at: value.finalized_at,
                created_at: value.created_at,
                updated_at: value.updated_at,
            },
            store_name: value.store_name,
            store_slug: value.store_slug,
            callback_url: value.callback_url,
            callback_secret: value.callback_secret,
        }
    }
}

impl From<ProviderWebhookEventRow> for ProviderWebhookEvent {
    fn from(value: ProviderWebhookEventRow) -> Self {
        Self {
            id: value.id,
            provider_name: value.provider_name,
            webhook_kind: ProviderWebhookKind::from(value.webhook_kind),
            merchant_id: value.merchant_id,
            provider_trx_id: value.provider_trx_id,
            partner_ref_no: value.partner_ref_no,
            payload_json: value.payload_json.0,
            is_verified: value.is_verified,
            verification_reason: value.verification_reason,
            is_processed: value.is_processed,
            processing_result: value.processing_result,
            processed_at: value.processed_at,
            created_at: value.created_at,
        }
    }
}

impl From<PaymentIdempotencyRow> for PaymentIdempotencyRecord {
    fn from(value: PaymentIdempotencyRow) -> Self {
        Self {
            id: value.id,
            store_id: value.store_id,
            idempotency_key: value.idempotency_key,
            request_hash: value.request_hash,
            status: PaymentIdempotencyStatus::from(value.status),
            response_status_code: value.response_status_code,
            response_body_json: value.response_body_json.map(|payload| payload.0),
            payment_id: value.payment_id,
            created_at: value.created_at,
            completed_at: value.completed_at,
            updated_at: value.updated_at,
        }
    }
}
