use std::sync::Arc;

use chrono::Utc;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::modules::balances::domain::entity::{
    BalanceBucket, BalanceSummaryDelta, LedgerDirection, NewStoreBalanceLedgerEntry,
    StoreBalanceEntryType,
};
use crate::modules::balances::domain::repository::StoreBalanceRepository;
use crate::modules::payouts::domain::entity::{
    NewPayoutRecord, PayoutListRow, PayoutPreviewResult, PayoutRecord, PayoutStatus,
    UpdatePayoutStatus,
};
use crate::modules::payouts::domain::repository::PayoutRepository;
use crate::modules::payments::application::provider::{
    InquiryBankRequest, PaymentProviderGateway, ProviderDisbursementType, TransferRequest,
};
use crate::modules::store_banks::domain::repository::StoreBankRepository;
use crate::shared::audit::{AuditLogEntry, AuditLogRepository};
use crate::shared::auth::{AuthenticatedUser, PlatformRole, StoreRole};
use crate::shared::error::AppError;
use crate::shared::money::{validate_payout_guard, withdraw_breakdown};

#[derive(Debug, Deserialize)]
pub struct PayoutPreviewRequest {
    pub requested_amount: i64,
    pub bank_account_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct PayoutConfirmRequest {
    pub requested_amount: i64,
    pub bank_account_id: Option<Uuid>,
}

pub struct PayoutService {
    payout_repository: Arc<dyn PayoutRepository>,
    balance_repository: Arc<dyn StoreBalanceRepository>,
    bank_repository: Arc<dyn StoreBankRepository>,
    provider: Arc<dyn PaymentProviderGateway>,
    audit_repository: Arc<dyn AuditLogRepository>,
}

impl PayoutService {
    pub fn new(
        payout_repository: Arc<dyn PayoutRepository>,
        balance_repository: Arc<dyn StoreBalanceRepository>,
        bank_repository: Arc<dyn StoreBankRepository>,
        provider: Arc<dyn PaymentProviderGateway>,
        audit_repository: Arc<dyn AuditLogRepository>,
    ) -> Self {
        Self {
            payout_repository,
            balance_repository,
            bank_repository,
            provider,
            audit_repository,
        }
    }

    pub async fn preview_payout(
        &self,
        store_id: Uuid,
        request: PayoutPreviewRequest,
        actor: &AuthenticatedUser,
    ) -> Result<PayoutPreviewResult, AppError> {
        ensure_payout_preview_access(actor, store_id)?;

        let balance = self
            .balance_repository
            .fetch_store_balance_summary(store_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Store balance not found".into()))?;

        let withdrawable = balance.withdrawable_balance();

        let bank_secret = match request.bank_account_id {
            Some(bank_id) => self
                .bank_repository
                .find_account_with_secret(store_id, bank_id)
                .await?
                .ok_or_else(|| AppError::NotFound("Bank account not found".into()))?,
            None => self
                .bank_repository
                .find_default_account_with_secret(store_id)
                .await?
                .ok_or_else(|| AppError::NotFound("Default bank account not found".into()))?,
        };

        let inquiry = self
            .provider
            .inquiry_bank(InquiryBankRequest {
                amount: request.requested_amount,
                bank_code: bank_secret.account.bank_code.clone(),
                account_number: bank_secret.account_number_plaintext.clone(),
                disbursement_type: ProviderDisbursementType::Instant,
            })
            .await?;

        let breakdown = withdraw_breakdown(request.requested_amount, inquiry.fee);

        validate_payout_guard(&breakdown, withdrawable).map_err(|money_error| {
            AppError::BadRequest(format!("Payout validation failed: {money_error}"))
        })?;

        Ok(PayoutPreviewResult {
            requested_amount: breakdown.requested_amount,
            platform_fee_bps: breakdown.platform_fee_bps,
            platform_fee_amount: breakdown.platform_fee_amount,
            provider_fee_amount: breakdown.provider_fee_amount,
            net_disbursed_amount: breakdown.net_disbursed_amount,
            bank_code: bank_secret.account.bank_code,
            bank_name: bank_secret.account.bank_name,
            account_holder_name: bank_secret.account.account_holder_name,
            account_number_last4: bank_secret.account.account_number_last4,
            withdrawable_balance: withdrawable,
            partner_ref_no: inquiry.partner_ref_no,
            inquiry_id: inquiry.inquiry_id,
        })
    }

    pub async fn confirm_payout(
        &self,
        store_id: Uuid,
        request: PayoutConfirmRequest,
        actor: &AuthenticatedUser,
    ) -> Result<PayoutRecord, AppError> {
        ensure_payout_create_access(actor, store_id)?;

        // Step 1: Fetch balance and bank account
        let balance = self
            .balance_repository
            .fetch_store_balance_summary(store_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Store balance not found".into()))?;

        let withdrawable = balance.withdrawable_balance();

        let bank_secret = match request.bank_account_id {
            Some(bank_id) => self
                .bank_repository
                .find_account_with_secret(store_id, bank_id)
                .await?
                .ok_or_else(|| AppError::NotFound("Bank account not found".into()))?,
            None => self
                .bank_repository
                .find_default_account_with_secret(store_id)
                .await?
                .ok_or_else(|| AppError::NotFound("Default bank account not found".into()))?,
        };

        // Step 2: Inquiry provider for real fee
        let inquiry = self
            .provider
            .inquiry_bank(InquiryBankRequest {
                amount: request.requested_amount,
                bank_code: bank_secret.account.bank_code.clone(),
                account_number: bank_secret.account_number_plaintext.clone(),
                disbursement_type: ProviderDisbursementType::Instant,
            })
            .await?;

        let breakdown = withdraw_breakdown(request.requested_amount, inquiry.fee);

        validate_payout_guard(&breakdown, withdrawable).map_err(|money_error| {
            AppError::BadRequest(format!("Payout validation failed: {money_error}"))
        })?;

        let now = Utc::now();

        // Step 3: Reserve balance
        self.balance_repository
            .apply_summary_delta(
                store_id,
                BalanceSummaryDelta {
                    pending_delta: 0,
                    settled_delta: 0,
                    reserved_delta: request.requested_amount,
                },
                now,
            )
            .await?;

        // Step 4: Insert reserve ledger entry
        self.balance_repository
            .insert_ledger_entry(NewStoreBalanceLedgerEntry {
                store_id,
                related_type: "payout".to_string(),
                related_id: None, // will be updated conceptually once payout row exists
                entry_type: StoreBalanceEntryType::PayoutReserveSettled,
                amount: request.requested_amount,
                direction: LedgerDirection::Debit,
                balance_bucket: BalanceBucket::Reserved,
                description: Some(format!(
                    "Reserve {} for payout to {}",
                    request.requested_amount, bank_secret.account.account_number_last4
                )),
                created_at: now,
            })
            .await?;

        // Step 5: Insert payout record
        let payout = self
            .payout_repository
            .insert_payout(NewPayoutRecord {
                store_id,
                bank_account_id: bank_secret.account.id,
                requested_by_user_id: actor.user_id,
                requested_amount: breakdown.requested_amount,
                platform_withdraw_fee_bps: breakdown.platform_fee_bps as i32,
                platform_withdraw_fee_amount: breakdown.platform_fee_amount,
                provider_withdraw_fee_amount: breakdown.provider_fee_amount,
                net_disbursed_amount: breakdown.net_disbursed_amount,
                provider_partner_ref_no: Some(inquiry.partner_ref_no.clone()),
                provider_inquiry_id: Some(inquiry.inquiry_id.to_string()),
                status: PayoutStatus::PendingProvider,
                created_at: now,
            })
            .await?;

        // Step 6: Call provider transfer
        let transfer_result = self
            .provider
            .transfer(TransferRequest {
                amount: breakdown.requested_amount,
                bank_code: bank_secret.account.bank_code.clone(),
                account_number: bank_secret.account_number_plaintext.clone(),
                disbursement_type: ProviderDisbursementType::Instant,
                inquiry_id: inquiry.inquiry_id,
            })
            .await;

        match transfer_result {
            Ok(_result) => {
                // Transfer accepted — payout stays pending_provider, webhook will finalize
                self.write_audit_log(
                    actor.user_id,
                    "payout.create",
                    payout.id,
                    json!({
                        "store_id": store_id,
                        "requested_amount": breakdown.requested_amount,
                        "platform_fee": breakdown.platform_fee_amount,
                        "provider_fee": breakdown.provider_fee_amount,
                        "net_disbursed": breakdown.net_disbursed_amount,
                        "bank_code": bank_secret.account.bank_code,
                        "account_last4": bank_secret.account.account_number_last4,
                        "partner_ref_no": inquiry.partner_ref_no,
                    }),
                )
                .await?;

                Ok(payout)
            }
            Err(transfer_error) => {
                // Step 7: Release reserve on failure
                let _ = self
                    .balance_repository
                    .apply_summary_delta(
                        store_id,
                        BalanceSummaryDelta {
                            pending_delta: 0,
                            settled_delta: 0,
                            reserved_delta: -request.requested_amount,
                        },
                        Utc::now(),
                    )
                    .await;

                let _ = self
                    .balance_repository
                    .insert_ledger_entry(NewStoreBalanceLedgerEntry {
                        store_id,
                        related_type: "payout".to_string(),
                        related_id: Some(payout.id),
                        entry_type: StoreBalanceEntryType::PayoutFailedReleaseReserve,
                        amount: request.requested_amount,
                        direction: LedgerDirection::Credit,
                        balance_bucket: BalanceBucket::Reserved,
                        description: Some(format!(
                            "Release reserve for failed payout {}",
                            payout.id
                        )),
                        created_at: Utc::now(),
                    })
                    .await;

                // Update payout to failed
                let _ = self
                    .payout_repository
                    .update_status(UpdatePayoutStatus {
                        payout_id: payout.id,
                        store_id,
                        new_status: PayoutStatus::Failed,
                        failure_reason: Some(transfer_error.to_string()),
                        updated_at: Utc::now(),
                    })
                    .await;

                self.write_audit_log(
                    actor.user_id,
                    "payout.failed",
                    payout.id,
                    json!({
                        "store_id": store_id,
                        "reason": transfer_error.to_string(),
                    }),
                )
                .await?;

                Err(AppError::BadRequest(format!(
                    "Payout transfer failed: {transfer_error}"
                )))
            }
        }
    }

    pub async fn list_payouts(
        &self,
        store_id: Uuid,
        limit: i64,
        offset: i64,
        actor: &AuthenticatedUser,
    ) -> Result<Vec<PayoutListRow>, AppError> {
        ensure_payout_read_access(actor, store_id)?;

        let limit = limit.clamp(1, 100);
        let offset = offset.max(0);

        self.payout_repository
            .list_by_store(store_id, limit, offset)
            .await
            .map_err(Into::into)
    }

    pub async fn get_payout_detail(
        &self,
        store_id: Uuid,
        payout_id: Uuid,
        actor: &AuthenticatedUser,
    ) -> Result<PayoutRecord, AppError> {
        ensure_payout_read_access(actor, store_id)?;

        self.payout_repository
            .find_by_id(store_id, payout_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Payout not found".into()))
    }

    async fn write_audit_log(
        &self,
        actor_user_id: Uuid,
        action: &str,
        target_id: Uuid,
        payload_json: serde_json::Value,
    ) -> Result<(), AppError> {
        self.audit_repository
            .insert(AuditLogEntry {
                actor_user_id: Some(actor_user_id),
                action: action.to_string(),
                target_type: Some("store_payout_request".to_string()),
                target_id: Some(target_id),
                payload_json,
            })
            .await?;

        Ok(())
    }
}

fn ensure_payout_preview_access(
    actor: &AuthenticatedUser,
    store_id: Uuid,
) -> Result<(), AppError> {
    if actor.platform_role == PlatformRole::Dev {
        return Ok(());
    }

    match actor.memberships.get(&store_id) {
        Some(StoreRole::Owner) => Ok(()),
        _ => Err(AppError::Forbidden(
            "You do not have permission to preview payouts".into(),
        )),
    }
}

fn ensure_payout_create_access(
    actor: &AuthenticatedUser,
    store_id: Uuid,
) -> Result<(), AppError> {
    if actor.platform_role == PlatformRole::Dev {
        return Ok(());
    }

    match actor.memberships.get(&store_id) {
        Some(StoreRole::Owner) => Ok(()),
        _ => Err(AppError::Forbidden(
            "You do not have permission to create payouts".into(),
        )),
    }
}

fn ensure_payout_read_access(
    actor: &AuthenticatedUser,
    store_id: Uuid,
) -> Result<(), AppError> {
    if actor.platform_role == PlatformRole::Dev
        || actor.platform_role == PlatformRole::Superadmin
    {
        return Ok(());
    }

    match actor.memberships.get(&store_id) {
        Some(StoreRole::Owner | StoreRole::Manager) => Ok(()),
        _ => {
            // Check platform role capabilities for scoped read
            if actor.platform_role == PlatformRole::Admin
                || actor.platform_role == PlatformRole::User
            {
                if actor.memberships.contains_key(&store_id) {
                    return Ok(());
                }
            }
            Err(AppError::Forbidden(
                "You do not have permission to view payouts".into(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use chrono::Utc;

    use super::*;
    use crate::modules::balances::domain::entity::{
        StoreBalanceLedgerEntry, StoreBalanceSnapshot, StoreBalanceSummary,
    };
    use crate::modules::payments::application::provider::{
        CheckDisbursementStatusRequest, CheckPaymentStatusRequest, CheckedDisbursementStatus,
        CheckedPaymentStatus, GenerateQrisRequest, GeneratedQris, GetBalanceRequest,
        InquiryBankResult, ProviderBalanceSnapshot, TransferResult,
    };
    use crate::modules::store_banks::domain::entity::{
        StoreBankAccountSecret, StoreBankAccountSummary, StoreBankStoreProfile,
        StoreBankVerificationStatus, NewStoreBankAccountRecord,
    };

    // ---- Mock PayoutRepository ----
    #[derive(Default)]
    struct MockPayoutRepository {
        payouts: Mutex<Vec<PayoutRecord>>,
    }

    #[async_trait]
    impl PayoutRepository for MockPayoutRepository {
        async fn insert_payout(&self, record: NewPayoutRecord) -> anyhow::Result<PayoutRecord> {
            let payout = PayoutRecord {
                id: Uuid::new_v4(),
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
                created_at: record.created_at,
                updated_at: record.created_at,
            };
            self.payouts.lock().unwrap().push(payout.clone());
            Ok(payout)
        }

        async fn update_status(
            &self,
            update: UpdatePayoutStatus,
        ) -> anyhow::Result<Option<PayoutRecord>> {
            let mut payouts = self.payouts.lock().unwrap();
            if let Some(payout) = payouts.iter_mut().find(|p| p.id == update.payout_id) {
                payout.status = update.new_status;
                payout.failure_reason = update.failure_reason;
                payout.updated_at = update.updated_at;
                return Ok(Some(payout.clone()));
            }
            Ok(None)
        }

        async fn find_by_id(
            &self,
            store_id: Uuid,
            payout_id: Uuid,
        ) -> anyhow::Result<Option<PayoutRecord>> {
            Ok(self
                .payouts
                .lock()
                .unwrap()
                .iter()
                .find(|p| p.id == payout_id && p.store_id == store_id)
                .cloned())
        }

        async fn list_by_store(
            &self,
            store_id: Uuid,
            _limit: i64,
            _offset: i64,
        ) -> anyhow::Result<Vec<PayoutListRow>> {
            Ok(self
                .payouts
                .lock()
                .unwrap()
                .iter()
                .filter(|p| p.store_id == store_id)
                .map(|p| PayoutListRow {
                    id: p.id,
                    store_id: p.store_id,
                    requested_amount: p.requested_amount,
                    platform_withdraw_fee_amount: p.platform_withdraw_fee_amount,
                    provider_withdraw_fee_amount: p.provider_withdraw_fee_amount,
                    net_disbursed_amount: p.net_disbursed_amount,
                    status: p.status,
                    bank_name: "Mock Bank".into(),
                    account_number_last4: "7890".into(),
                    account_holder_name: "Test Owner".into(),
                    created_at: p.created_at,
                })
                .collect())
        }
    }

    // ---- Mock StoreBalanceRepository ----
    #[derive(Default)]
    struct MockBalanceRepository {
        summary: Mutex<Option<StoreBalanceSummary>>,
        ledger_entries: Mutex<Vec<NewStoreBalanceLedgerEntry>>,
    }

    #[async_trait]
    impl StoreBalanceRepository for MockBalanceRepository {
        async fn fetch_store_balance_snapshot(
            &self,
            _store_id: Uuid,
            _user_scope: Option<Uuid>,
            _global_access: bool,
        ) -> anyhow::Result<Option<StoreBalanceSnapshot>> {
            Ok(self.summary.lock().unwrap().as_ref().map(|s| {
                StoreBalanceSnapshot {
                    store_id: s.store_id,
                    pending_balance: s.pending_balance,
                    settled_balance: s.settled_balance,
                    reserved_settled_balance: s.reserved_settled_balance,
                    withdrawable_balance: s.withdrawable_balance(),
                    updated_at: s.updated_at,
                }
            }))
        }

        async fn fetch_store_balance_summary(
            &self,
            _store_id: Uuid,
        ) -> anyhow::Result<Option<StoreBalanceSummary>> {
            Ok(self.summary.lock().unwrap().clone())
        }

        async fn apply_summary_delta(
            &self,
            store_id: Uuid,
            delta: BalanceSummaryDelta,
            updated_at: chrono::DateTime<chrono::Utc>,
        ) -> anyhow::Result<StoreBalanceSummary> {
            let mut guard = self.summary.lock().unwrap();
            let summary = guard.get_or_insert_with(|| StoreBalanceSummary {
                store_id,
                pending_balance: 0,
                settled_balance: 0,
                reserved_settled_balance: 0,
                updated_at,
            });
            summary.pending_balance += delta.pending_delta;
            summary.settled_balance += delta.settled_delta;
            summary.reserved_settled_balance += delta.reserved_delta;
            summary.updated_at = updated_at;
            Ok(summary.clone())
        }

        async fn insert_ledger_entry(
            &self,
            entry: NewStoreBalanceLedgerEntry,
        ) -> anyhow::Result<StoreBalanceLedgerEntry> {
            self.ledger_entries.lock().unwrap().push(entry.clone());
            Ok(StoreBalanceLedgerEntry {
                id: Uuid::new_v4(),
                store_id: entry.store_id,
                related_type: entry.related_type,
                related_id: entry.related_id,
                entry_type: entry.entry_type,
                amount: entry.amount,
                direction: entry.direction,
                balance_bucket: entry.balance_bucket,
                description: entry.description,
                created_at: entry.created_at,
            })
        }
    }

    // ---- Mock StoreBankRepository ----
    struct MockBankRepository {
        account: Mutex<Option<StoreBankAccountSecret>>,
    }

    impl Default for MockBankRepository {
        fn default() -> Self {
            Self {
                account: Mutex::new(None),
            }
        }
    }

    #[async_trait]
    impl StoreBankRepository for MockBankRepository {
        async fn find_store_profile(
            &self,
            _store_id: Uuid,
        ) -> anyhow::Result<Option<StoreBankStoreProfile>> {
            Ok(None)
        }

        async fn list_by_store(
            &self,
            _store_id: Uuid,
        ) -> anyhow::Result<Vec<StoreBankAccountSummary>> {
            Ok(vec![])
        }

        async fn insert_verified_account(
            &self,
            _record: NewStoreBankAccountRecord,
        ) -> anyhow::Result<StoreBankAccountSummary> {
            unreachable!()
        }

        async fn set_default_bank(
            &self,
            _store_id: Uuid,
            _bank_account_id: Uuid,
            _updated_at: chrono::DateTime<Utc>,
        ) -> anyhow::Result<Option<StoreBankAccountSummary>> {
            unreachable!()
        }

        async fn find_account_with_secret(
            &self,
            _store_id: Uuid,
            _bank_account_id: Uuid,
        ) -> anyhow::Result<Option<StoreBankAccountSecret>> {
            Ok(self.account.lock().unwrap().clone())
        }

        async fn find_default_account_with_secret(
            &self,
            _store_id: Uuid,
        ) -> anyhow::Result<Option<StoreBankAccountSecret>> {
            Ok(self.account.lock().unwrap().clone())
        }
    }

    // ---- Mock Provider ----
    #[derive(Default)]
    struct MockProvider {
        inquiry_fee: Mutex<i64>,
        transfer_should_fail: Mutex<bool>,
    }

    #[async_trait]
    impl PaymentProviderGateway for MockProvider {
        async fn generate_qris(
            &self,
            _request: GenerateQrisRequest,
        ) -> Result<GeneratedQris, AppError> {
            unreachable!()
        }

        async fn check_payment_status(
            &self,
            _request: CheckPaymentStatusRequest,
        ) -> Result<CheckedPaymentStatus, AppError> {
            unreachable!()
        }

        async fn inquiry_bank(
            &self,
            request: InquiryBankRequest,
        ) -> Result<InquiryBankResult, AppError> {
            let fee = *self.inquiry_fee.lock().unwrap();
            Ok(InquiryBankResult {
                account_number: request.account_number,
                account_name: "Test Owner".into(),
                bank_code: request.bank_code,
                bank_name: "Mock Bank".into(),
                partner_ref_no: "partner-ref-123".into(),
                vendor_ref_no: None,
                amount: request.amount,
                fee,
                inquiry_id: 42,
            })
        }

        async fn transfer(&self, _request: TransferRequest) -> Result<TransferResult, AppError> {
            if *self.transfer_should_fail.lock().unwrap() {
                return Err(AppError::BadRequest(
                    "Provider transfer rejected".into(),
                ));
            }
            Ok(TransferResult { accepted: true })
        }

        async fn check_disbursement_status(
            &self,
            _request: CheckDisbursementStatusRequest,
        ) -> Result<CheckedDisbursementStatus, AppError> {
            unreachable!()
        }

        async fn get_balance(
            &self,
            _request: GetBalanceRequest,
        ) -> Result<ProviderBalanceSnapshot, AppError> {
            unreachable!()
        }
    }

    // ---- Mock AuditLogRepository ----
    #[derive(Default)]
    struct MockAuditRepository {
        entries: Mutex<Vec<AuditLogEntry>>,
    }

    #[async_trait]
    impl AuditLogRepository for MockAuditRepository {
        async fn insert(&self, entry: AuditLogEntry) -> anyhow::Result<()> {
            self.entries.lock().unwrap().push(entry);
            Ok(())
        }
    }

    // ---- helpers ----
    fn owner_actor(store_id: Uuid) -> AuthenticatedUser {
        AuthenticatedUser {
            user_id: Uuid::new_v4(),
            platform_role: PlatformRole::User,
            memberships: HashMap::from([(store_id, StoreRole::Owner)]),
        }
    }

    fn dev_actor() -> AuthenticatedUser {
        AuthenticatedUser {
            user_id: Uuid::new_v4(),
            platform_role: PlatformRole::Dev,
            memberships: HashMap::new(),
        }
    }

    fn viewer_actor(store_id: Uuid) -> AuthenticatedUser {
        AuthenticatedUser {
            user_id: Uuid::new_v4(),
            platform_role: PlatformRole::User,
            memberships: HashMap::from([(store_id, StoreRole::Viewer)]),
        }
    }

    fn make_bank_secret(store_id: Uuid) -> StoreBankAccountSecret {
        StoreBankAccountSecret {
            account: StoreBankAccountSummary {
                id: Uuid::new_v4(),
                store_id,
                owner_user_id: Uuid::new_v4(),
                bank_code: "014".into(),
                bank_name: "BCA".into(),
                account_holder_name: "Test Owner".into(),
                account_number_last4: "7890".into(),
                is_default: true,
                verification_status: StoreBankVerificationStatus::Verified,
                verified_at: Some(Utc::now()),
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
            account_number_plaintext: "1234567890".into(),
        }
    }

    fn build_service(
        store_id: Uuid,
        settled_balance: i64,
        provider_fee: i64,
        transfer_fails: bool,
    ) -> (
        PayoutService,
        Arc<MockPayoutRepository>,
        Arc<MockBalanceRepository>,
    ) {
        let payout_repo = Arc::new(MockPayoutRepository::default());
        let balance_repo = Arc::new(MockBalanceRepository::default());
        balance_repo
            .summary
            .lock()
            .unwrap()
            .replace(StoreBalanceSummary {
                store_id,
                pending_balance: 0,
                settled_balance,
                reserved_settled_balance: 0,
                updated_at: Utc::now(),
            });

        let bank_repo = Arc::new(MockBankRepository::default());
        bank_repo
            .account
            .lock()
            .unwrap()
            .replace(make_bank_secret(store_id));

        let provider = Arc::new(MockProvider::default());
        *provider.inquiry_fee.lock().unwrap() = provider_fee;
        *provider.transfer_should_fail.lock().unwrap() = transfer_fails;

        let audit_repo = Arc::new(MockAuditRepository::default());

        let service = PayoutService::new(
            payout_repo.clone(),
            balance_repo.clone(),
            bank_repo,
            provider,
            audit_repo,
        );

        (service, payout_repo, balance_repo)
    }

    // ---- Tests ----

    #[tokio::test]
    async fn preview_returns_correct_breakdown() {
        let store_id = Uuid::new_v4();
        let (service, _, _) = build_service(store_id, 1_000_000, 1_800, false);

        let result = service
            .preview_payout(
                store_id,
                PayoutPreviewRequest {
                    requested_amount: 100_000,
                    bank_account_id: None,
                },
                &owner_actor(store_id),
            )
            .await
            .unwrap();

        assert_eq!(result.requested_amount, 100_000);
        assert_eq!(result.platform_fee_bps, 1200);
        assert_eq!(result.platform_fee_amount, 12_000);
        assert_eq!(result.provider_fee_amount, 1_800);
        assert_eq!(result.net_disbursed_amount, 86_200);
        assert_eq!(result.account_number_last4, "7890");
    }

    #[tokio::test]
    async fn preview_rejects_non_positive_amount() {
        let store_id = Uuid::new_v4();
        let (service, _, _) = build_service(store_id, 1_000_000, 1_800, false);

        let error = service
            .preview_payout(
                store_id,
                PayoutPreviewRequest {
                    requested_amount: 0,
                    bank_account_id: None,
                },
                &owner_actor(store_id),
            )
            .await
            .unwrap_err();

        assert!(matches!(error, AppError::BadRequest(_)));
    }

    #[tokio::test]
    async fn preview_rejects_when_exceeds_withdrawable() {
        let store_id = Uuid::new_v4();
        let (service, _, _) = build_service(store_id, 50_000, 1_800, false);

        let error = service
            .preview_payout(
                store_id,
                PayoutPreviewRequest {
                    requested_amount: 100_000,
                    bank_account_id: None,
                },
                &owner_actor(store_id),
            )
            .await
            .unwrap_err();

        assert!(matches!(error, AppError::BadRequest(_)));
    }

    #[tokio::test]
    async fn preview_rejects_negative_net() {
        let store_id = Uuid::new_v4();
        // Provider fee so large that net would be negative
        let (service, _, _) = build_service(store_id, 1_000_000, 100_000, false);

        let error = service
            .preview_payout(
                store_id,
                PayoutPreviewRequest {
                    requested_amount: 10_000,
                    bank_account_id: None,
                },
                &owner_actor(store_id),
            )
            .await
            .unwrap_err();

        assert!(matches!(error, AppError::BadRequest(_)));
    }

    #[tokio::test]
    async fn confirm_reserves_balance_and_creates_payout() {
        let store_id = Uuid::new_v4();
        let (service, payout_repo, balance_repo) =
            build_service(store_id, 1_000_000, 1_800, false);

        let payout = service
            .confirm_payout(
                store_id,
                PayoutConfirmRequest {
                    requested_amount: 100_000,
                    bank_account_id: None,
                },
                &owner_actor(store_id),
            )
            .await
            .unwrap();

        assert_eq!(payout.status, PayoutStatus::PendingProvider);
        assert_eq!(payout.requested_amount, 100_000);
        assert_eq!(payout.net_disbursed_amount, 86_200);

        let payouts = payout_repo.payouts.lock().unwrap();
        assert_eq!(payouts.len(), 1);

        let balance = balance_repo.summary.lock().unwrap();
        let summary = balance.as_ref().unwrap();
        assert_eq!(summary.reserved_settled_balance, 100_000);

        let ledger = balance_repo.ledger_entries.lock().unwrap();
        assert_eq!(ledger.len(), 1);
        assert_eq!(
            ledger[0].entry_type,
            StoreBalanceEntryType::PayoutReserveSettled
        );
    }

    #[tokio::test]
    async fn confirm_releases_reserve_on_transfer_failure() {
        let store_id = Uuid::new_v4();
        let (service, payout_repo, balance_repo) =
            build_service(store_id, 1_000_000, 1_800, true);

        let error = service
            .confirm_payout(
                store_id,
                PayoutConfirmRequest {
                    requested_amount: 100_000,
                    bank_account_id: None,
                },
                &owner_actor(store_id),
            )
            .await
            .unwrap_err();

        assert!(matches!(error, AppError::BadRequest(_)));

        // Reserve should be released
        let balance = balance_repo.summary.lock().unwrap();
        let summary = balance.as_ref().unwrap();
        assert_eq!(summary.reserved_settled_balance, 0);

        // Payout status should be failed
        let payouts = payout_repo.payouts.lock().unwrap();
        assert_eq!(payouts.len(), 1);
        assert_eq!(payouts[0].status, PayoutStatus::Failed);

        // Should have reserve + release ledger entries
        let ledger = balance_repo.ledger_entries.lock().unwrap();
        assert_eq!(ledger.len(), 2);
        assert_eq!(
            ledger[0].entry_type,
            StoreBalanceEntryType::PayoutReserveSettled
        );
        assert_eq!(
            ledger[1].entry_type,
            StoreBalanceEntryType::PayoutFailedReleaseReserve
        );
    }

    #[tokio::test]
    async fn only_owner_and_dev_can_preview() {
        let store_id = Uuid::new_v4();
        let (service, _, _) = build_service(store_id, 1_000_000, 1_800, false);

        // Owner can preview
        assert!(service
            .preview_payout(
                store_id,
                PayoutPreviewRequest {
                    requested_amount: 50_000,
                    bank_account_id: None,
                },
                &owner_actor(store_id),
            )
            .await
            .is_ok());

        // Dev can preview
        assert!(service
            .preview_payout(
                store_id,
                PayoutPreviewRequest {
                    requested_amount: 50_000,
                    bank_account_id: None,
                },
                &dev_actor(),
            )
            .await
            .is_ok());

        // Viewer cannot preview
        let error = service
            .preview_payout(
                store_id,
                PayoutPreviewRequest {
                    requested_amount: 50_000,
                    bank_account_id: None,
                },
                &viewer_actor(store_id),
            )
            .await
            .unwrap_err();
        assert!(matches!(error, AppError::Forbidden(_)));
    }

    #[tokio::test]
    async fn only_owner_and_dev_can_confirm() {
        let store_id = Uuid::new_v4();
        let (service, _, _) = build_service(store_id, 1_000_000, 1_800, false);

        // Viewer cannot confirm
        let error = service
            .confirm_payout(
                store_id,
                PayoutConfirmRequest {
                    requested_amount: 50_000,
                    bank_account_id: None,
                },
                &viewer_actor(store_id),
            )
            .await
            .unwrap_err();
        assert!(matches!(error, AppError::Forbidden(_)));
    }

    #[tokio::test]
    async fn list_respects_payout_read_capability() {
        let store_id = Uuid::new_v4();
        let (service, _, _) = build_service(store_id, 1_000_000, 1_800, false);

        // Owner can list
        assert!(service
            .list_payouts(store_id, 10, 0, &owner_actor(store_id))
            .await
            .is_ok());

        // Dev can list
        assert!(service
            .list_payouts(store_id, 10, 0, &dev_actor())
            .await
            .is_ok());

        // User without membership cannot list
        let no_membership = AuthenticatedUser {
            user_id: Uuid::new_v4(),
            platform_role: PlatformRole::User,
            memberships: HashMap::new(),
        };
        let error = service
            .list_payouts(store_id, 10, 0, &no_membership)
            .await
            .unwrap_err();
        assert!(matches!(error, AppError::Forbidden(_)));
    }
}
