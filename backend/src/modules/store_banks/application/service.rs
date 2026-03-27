use std::sync::Arc;

use chrono::Utc;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::modules::payments::application::provider::{
    InquiryBankRequest, PaymentProviderGateway, ProviderDisbursementType,
};
use crate::modules::store_banks::domain::entity::{
    NewStoreBankAccountRecord, StoreBankAccountSecret, StoreBankAccountSummary, StoreBankInquiry,
    StoreBankStoreProfile, StoreBankVerificationStatus,
};
use crate::modules::store_banks::domain::repository::{
    StoreBankInquiryCache, StoreBankRepository,
};
use crate::shared::audit::{AuditLogEntry, AuditLogRepository};
use crate::shared::auth::{AuthenticatedUser, PlatformRole, StoreRole};
use crate::shared::error::AppError;

// Provider inquiry currently rejects instant disbursement validations below Rp25.000.
// Store bank verification is not a real payout, but it still must satisfy the provider minimum.
const BANK_INQUIRY_PLACEHOLDER_AMOUNT: i64 = 25_000;

#[derive(Debug, Deserialize)]
pub struct BankAccountInquiryRequest {
    pub bank_code: String,
    pub account_number: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateStoreBankAccountRequest {
    pub bank_code: String,
    pub account_number: String,
    #[serde(default)]
    pub is_default: bool,
}

pub struct StoreBankService {
    repository: Arc<dyn StoreBankRepository>,
    inquiry_cache: Arc<dyn StoreBankInquiryCache>,
    provider: Arc<dyn PaymentProviderGateway>,
    audit_repository: Arc<dyn AuditLogRepository>,
}

impl StoreBankService {
    pub fn new(
        repository: Arc<dyn StoreBankRepository>,
        inquiry_cache: Arc<dyn StoreBankInquiryCache>,
        provider: Arc<dyn PaymentProviderGateway>,
        audit_repository: Arc<dyn AuditLogRepository>,
    ) -> Self {
        Self {
            repository,
            inquiry_cache,
            provider,
            audit_repository,
        }
    }

    pub async fn list_accounts(
        &self,
        store_id: Uuid,
        actor: &AuthenticatedUser,
    ) -> Result<Vec<StoreBankAccountSummary>, AppError> {
        let _store = self.ensure_store_profile(store_id).await?;
        ensure_bank_read_access(actor, store_id)?;

        self.repository
            .list_by_store(store_id)
            .await
            .map_err(Into::into)
    }

    pub async fn inquiry_account(
        &self,
        store_id: Uuid,
        request: BankAccountInquiryRequest,
        actor: &AuthenticatedUser,
    ) -> Result<StoreBankInquiry, AppError> {
        let _store = self.ensure_store_profile(store_id).await?;
        ensure_bank_manage_access(actor, store_id)?;

        let bank_code = normalize_bank_code(&request.bank_code)?;
        let account_number = normalize_account_number(&request.account_number)?;
        let provider_result = self
            .provider
            .inquiry_bank(InquiryBankRequest {
                amount: BANK_INQUIRY_PLACEHOLDER_AMOUNT,
                bank_code: bank_code.clone(),
                account_number: account_number.clone(),
                disbursement_type: ProviderDisbursementType::Instant,
            })
            .await?;

        if provider_result.account_number != account_number {
            return Err(AppError::Conflict(
                "Provider inquiry returned a different account number".into(),
            ));
        }

        let inquiry = StoreBankInquiry {
            bank_code: provider_result.bank_code,
            bank_name: provider_result.bank_name,
            account_holder_name: provider_result.account_name,
            account_number_last4: last4(&account_number),
            provider_fee_amount: provider_result.fee,
            partner_ref_no: provider_result.partner_ref_no,
            vendor_ref_no: provider_result.vendor_ref_no,
            inquiry_id: provider_result.inquiry_id,
        };

        self.inquiry_cache
            .remember_verified_inquiry(actor.user_id, store_id, &bank_code, &account_number, &inquiry)
            .await?;

        Ok(inquiry)
    }

    pub async fn create_account(
        &self,
        store_id: Uuid,
        request: CreateStoreBankAccountRequest,
        actor: &AuthenticatedUser,
    ) -> Result<StoreBankAccountSummary, AppError> {
        let store = self.ensure_store_profile(store_id).await?;
        ensure_bank_manage_access(actor, store_id)?;

        let bank_code = normalize_bank_code(&request.bank_code)?;
        let account_number = normalize_account_number(&request.account_number)?;
        let verified_inquiry = self
            .inquiry_cache
            .find_verified_inquiry(actor.user_id, store_id, &bank_code, &account_number)
            .await?;
        let verified_inquiry = verified_inquiry.ok_or_else(|| {
            AppError::BadRequest("Bank account must be verified before it can be saved".into())
        })?;

        let now = Utc::now();
        let bank = self
            .repository
            .insert_verified_account(NewStoreBankAccountRecord {
                id: Uuid::new_v4(),
                store_id,
                owner_user_id: store.owner_user_id,
                bank_code,
                bank_name: verified_inquiry.bank_name,
                account_holder_name: verified_inquiry.account_holder_name,
                account_number_plaintext: account_number.clone(),
                account_number_last4: last4(&account_number),
                is_default: request.is_default,
                verification_status: StoreBankVerificationStatus::Verified,
                verified_at: now,
                created_at: now,
                updated_at: now,
            })
            .await?;

        self.write_audit_log(
            actor.user_id,
            "store.bank.create",
            bank.id,
            json!({
                "store_id": store.store_id,
                "store_name": store.store_name,
                "bank_code": bank.bank_code,
                "bank_name": bank.bank_name,
                "account_holder_name": bank.account_holder_name,
                "account_number_last4": bank.account_number_last4,
                "is_default": bank.is_default,
            }),
        )
        .await?;

        Ok(bank)
    }

    pub async fn set_default_account(
        &self,
        store_id: Uuid,
        bank_account_id: Uuid,
        actor: &AuthenticatedUser,
    ) -> Result<StoreBankAccountSummary, AppError> {
        let store = self.ensure_store_profile(store_id).await?;
        ensure_bank_manage_access(actor, store_id)?;

        let bank = self
            .repository
            .set_default_bank(store_id, bank_account_id, Utc::now())
            .await?
            .ok_or_else(|| AppError::NotFound("Bank account not found".into()))?;

        self.write_audit_log(
            actor.user_id,
            "store.bank.set_default",
            bank.id,
            json!({
                "store_id": store.store_id,
                "store_name": store.store_name,
                "bank_code": bank.bank_code,
                "bank_name": bank.bank_name,
                "account_number_last4": bank.account_number_last4,
                "is_default": true,
            }),
        )
        .await?;

        Ok(bank)
    }

    pub async fn get_account_with_secret(
        &self,
        store_id: Uuid,
        bank_account_id: Uuid,
        actor: &AuthenticatedUser,
    ) -> Result<StoreBankAccountSecret, AppError> {
        let _store = self.ensure_store_profile(store_id).await?;
        ensure_bank_manage_access(actor, store_id)?;

        self.repository
            .find_account_with_secret(store_id, bank_account_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Bank account not found".into()))
    }

    pub async fn get_default_account_with_secret(
        &self,
        store_id: Uuid,
        actor: &AuthenticatedUser,
    ) -> Result<StoreBankAccountSecret, AppError> {
        let _store = self.ensure_store_profile(store_id).await?;
        ensure_bank_manage_access(actor, store_id)?;

        self.repository
            .find_default_account_with_secret(store_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Default bank account not found".into()))
    }

    async fn ensure_store_profile(
        &self,
        store_id: Uuid,
    ) -> Result<StoreBankStoreProfile, AppError> {
        self.repository
            .find_store_profile(store_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Store not found".into()))
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
                target_type: Some("store_bank_account".to_string()),
                target_id: Some(target_id),
                payload_json,
            })
            .await?;

        Ok(())
    }
}

fn normalize_bank_code(value: &str) -> Result<String, AppError> {
    let normalized = value.trim();
    if normalized.is_empty() {
        return Err(AppError::BadRequest("Bank code is required".into()));
    }

    Ok(normalized.to_string())
}

fn normalize_account_number(value: &str) -> Result<String, AppError> {
    let normalized = value.trim();
    if normalized.len() < 4 {
        return Err(AppError::BadRequest(
            "Account number must be at least 4 digits".into(),
        ));
    }
    if !normalized.chars().all(|character| character.is_ascii_digit()) {
        return Err(AppError::BadRequest(
            "Account number must contain digits only".into(),
        ));
    }

    Ok(normalized.to_string())
}

fn last4(account_number: &str) -> String {
    account_number
        .chars()
        .rev()
        .take(4)
        .collect::<String>()
        .chars()
        .rev()
        .collect()
}

fn ensure_bank_read_access(actor: &AuthenticatedUser, store_id: Uuid) -> Result<(), AppError> {
    if actor.platform_role == PlatformRole::Dev || actor.platform_role == PlatformRole::Superadmin {
        return Ok(());
    }

    match actor.memberships.get(&store_id) {
        Some(StoreRole::Owner) => Ok(()),
        _ => Err(AppError::Forbidden(
            "You do not have permission to view bank accounts".into(),
        )),
    }
}

fn ensure_bank_manage_access(actor: &AuthenticatedUser, store_id: Uuid) -> Result<(), AppError> {
    if actor.platform_role == PlatformRole::Dev {
        return Ok(());
    }

    match actor.memberships.get(&store_id) {
        Some(StoreRole::Owner) => Ok(()),
        _ => Err(AppError::Forbidden(
            "You do not have permission to manage bank accounts".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;

    use super::*;
    use crate::modules::payments::application::provider::{
        CheckDisbursementStatusRequest, CheckPaymentStatusRequest, CheckedDisbursementStatus,
        CheckedPaymentStatus, GenerateQrisRequest, GeneratedQris, GetBalanceRequest,
        InquiryBankResult, ProviderBalanceSnapshot, TransferRequest, TransferResult,
    };

    #[derive(Default)]
    struct MockStoreBankRepository {
        store_profile: Mutex<Option<StoreBankStoreProfile>>,
        accounts: Mutex<Vec<StoreBankAccountSummary>>,
    }

    #[async_trait]
    impl StoreBankRepository for MockStoreBankRepository {
        async fn find_store_profile(
            &self,
            _store_id: Uuid,
        ) -> anyhow::Result<Option<StoreBankStoreProfile>> {
            Ok(self.store_profile.lock().unwrap().clone())
        }

        async fn list_by_store(
            &self,
            _store_id: Uuid,
        ) -> anyhow::Result<Vec<StoreBankAccountSummary>> {
            Ok(self.accounts.lock().unwrap().clone())
        }

        async fn insert_verified_account(
            &self,
            record: NewStoreBankAccountRecord,
        ) -> anyhow::Result<StoreBankAccountSummary> {
            let summary = StoreBankAccountSummary {
                id: record.id,
                store_id: record.store_id,
                owner_user_id: record.owner_user_id,
                bank_code: record.bank_code,
                bank_name: record.bank_name,
                account_holder_name: record.account_holder_name,
                account_number_last4: record.account_number_last4,
                is_default: record.is_default,
                verification_status: record.verification_status,
                verified_at: Some(record.verified_at),
                created_at: record.created_at,
                updated_at: record.updated_at,
            };
            self.accounts.lock().unwrap().push(summary.clone());
            Ok(summary)
        }

        async fn set_default_bank(
            &self,
            _store_id: Uuid,
            bank_account_id: Uuid,
            updated_at: chrono::DateTime<Utc>,
        ) -> anyhow::Result<Option<StoreBankAccountSummary>> {
            let mut accounts = self.accounts.lock().unwrap();
            let target_index = accounts.iter().position(|account| account.id == bank_account_id);
            let Some(target_index) = target_index else {
                return Ok(None);
            };
            for account in accounts.iter_mut() {
                account.is_default = false;
                account.updated_at = updated_at;
            }
            accounts[target_index].is_default = true;
            Ok(Some(accounts[target_index].clone()))
        }

        async fn find_account_with_secret(
            &self,
            _store_id: Uuid,
            bank_account_id: Uuid,
        ) -> anyhow::Result<Option<StoreBankAccountSecret>> {
            Ok(self
                .accounts
                .lock()
                .unwrap()
                .iter()
                .find(|account| account.id == bank_account_id)
                .cloned()
                .map(|account| StoreBankAccountSecret {
                    account,
                    account_number_plaintext: "1234567890".into(),
                }))
        }

        async fn find_default_account_with_secret(
            &self,
            _store_id: Uuid,
        ) -> anyhow::Result<Option<StoreBankAccountSecret>> {
            Ok(self
                .accounts
                .lock()
                .unwrap()
                .iter()
                .find(|account| account.is_default)
                .cloned()
                .map(|account| StoreBankAccountSecret {
                    account,
                    account_number_plaintext: "1234567890".into(),
                }))
        }
    }

    #[derive(Default)]
    struct MockProvider {
        inquiry_call_count: Mutex<usize>,
        last_inquiry_amount: Mutex<Option<i64>>,
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
            *self.inquiry_call_count.lock().unwrap() += 1;
            self.last_inquiry_amount
                .lock()
                .unwrap()
                .replace(request.amount);
            Ok(InquiryBankResult {
                account_number: request.account_number,
                account_name: "Alice Owner".into(),
                bank_code: request.bank_code,
                bank_name: "Mock Bank".into(),
                partner_ref_no: "partner-ref".into(),
                vendor_ref_no: Some("vendor-ref".into()),
                amount: request.amount,
                fee: 1800,
                inquiry_id: 77,
            })
        }

        async fn transfer(&self, _request: TransferRequest) -> Result<TransferResult, AppError> {
            unreachable!()
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

    #[derive(Default)]
    struct MockAuditRepository {
        payloads: Mutex<Vec<serde_json::Value>>,
    }

    #[async_trait]
    impl AuditLogRepository for MockAuditRepository {
        async fn insert(&self, entry: AuditLogEntry) -> anyhow::Result<()> {
            self.payloads.lock().unwrap().push(entry.payload_json);
            Ok(())
        }
    }

    #[derive(Default)]
    struct MockInquiryCache {
        records: Mutex<HashMap<(Uuid, Uuid, String, String), StoreBankInquiry>>,
    }

    #[async_trait]
    impl StoreBankInquiryCache for MockInquiryCache {
        async fn remember_verified_inquiry(
            &self,
            actor_user_id: Uuid,
            store_id: Uuid,
            bank_code: &str,
            account_number: &str,
            inquiry: &StoreBankInquiry,
        ) -> anyhow::Result<()> {
            self.records.lock().unwrap().insert(
                (
                    actor_user_id,
                    store_id,
                    bank_code.to_string(),
                    account_number.to_string(),
                ),
                inquiry.clone(),
            );
            Ok(())
        }

        async fn find_verified_inquiry(
            &self,
            actor_user_id: Uuid,
            store_id: Uuid,
            bank_code: &str,
            account_number: &str,
        ) -> anyhow::Result<Option<StoreBankInquiry>> {
            Ok(self
                .records
                .lock()
                .unwrap()
                .get(&(
                    actor_user_id,
                    store_id,
                    bank_code.to_string(),
                    account_number.to_string(),
                ))
                .cloned())
        }
    }

    fn owner_actor(store_id: Uuid) -> AuthenticatedUser {
        AuthenticatedUser {
            user_id: Uuid::new_v4(),
            platform_role: PlatformRole::User,
            memberships: HashMap::from([(store_id, StoreRole::Owner)]),
        }
    }

    #[tokio::test]
    async fn inquiry_returns_last4_and_provider_name_without_full_account_echo() {
        let store_id = Uuid::new_v4();
        let repository = Arc::new(MockStoreBankRepository::default());
        repository.store_profile.lock().unwrap().replace(StoreBankStoreProfile {
            store_id,
            owner_user_id: Uuid::new_v4(),
            store_name: "Alpha".into(),
        });
        let provider = Arc::new(MockProvider::default());
        let service = StoreBankService::new(
            repository,
            Arc::new(MockInquiryCache::default()),
            provider.clone(),
            Arc::new(MockAuditRepository::default()),
        );

        let inquiry = service
            .inquiry_account(
                store_id,
                BankAccountInquiryRequest {
                    bank_code: "014".into(),
                    account_number: "1234567890".into(),
                },
                &owner_actor(store_id),
            )
            .await
            .unwrap();

        assert_eq!(inquiry.account_holder_name, "Alice Owner");
        assert_eq!(inquiry.account_number_last4, "7890");
        assert_eq!(inquiry.bank_name, "Mock Bank");
        assert_eq!(
            provider.last_inquiry_amount.lock().unwrap().as_ref().copied(),
            Some(BANK_INQUIRY_PLACEHOLDER_AMOUNT)
        );
    }

    #[tokio::test]
    async fn superadmin_is_read_only_for_bank_accounts() {
        let store_id = Uuid::new_v4();
        let repository = Arc::new(MockStoreBankRepository::default());
        repository.store_profile.lock().unwrap().replace(StoreBankStoreProfile {
            store_id,
            owner_user_id: Uuid::new_v4(),
            store_name: "Alpha".into(),
        });
        let service = StoreBankService::new(
            repository,
            Arc::new(MockInquiryCache::default()),
            Arc::new(MockProvider::default()),
            Arc::new(MockAuditRepository::default()),
        );
        let actor = AuthenticatedUser {
            user_id: Uuid::new_v4(),
            platform_role: PlatformRole::Superadmin,
            memberships: HashMap::new(),
        };

        assert!(service.list_accounts(store_id, &actor).await.is_ok());
        let error = service
            .create_account(
                store_id,
                CreateStoreBankAccountRequest {
                    bank_code: "014".into(),
                    account_number: "1234567890".into(),
                    is_default: true,
                },
                &actor,
            )
            .await
            .unwrap_err();
        assert!(matches!(error, AppError::Forbidden(_)));
    }

    #[tokio::test]
    async fn audit_payload_does_not_contain_plain_account_number() {
        let store_id = Uuid::new_v4();
        let repository = Arc::new(MockStoreBankRepository::default());
        repository.store_profile.lock().unwrap().replace(StoreBankStoreProfile {
            store_id,
            owner_user_id: Uuid::new_v4(),
            store_name: "Alpha".into(),
        });
        let audits = Arc::new(MockAuditRepository::default());
        let provider = Arc::new(MockProvider::default());
        let service = StoreBankService::new(
            repository,
            Arc::new(MockInquiryCache::default()),
            provider.clone(),
            audits.clone(),
        );
        let actor = owner_actor(store_id);

        service
            .inquiry_account(
                store_id,
                BankAccountInquiryRequest {
                    bank_code: "014".into(),
                    account_number: "1234567890".into(),
                },
                &actor,
            )
            .await
            .unwrap();

        let bank = service
            .create_account(
                store_id,
                CreateStoreBankAccountRequest {
                    bank_code: "014".into(),
                    account_number: "1234567890".into(),
                    is_default: true,
                },
                &actor,
            )
            .await
            .unwrap();

        assert_eq!(bank.account_number_last4, "7890");
        assert_eq!(*provider.inquiry_call_count.lock().unwrap(), 1);
        let audit_payload = audits.payloads.lock().unwrap()[0].to_string();
        assert!(!audit_payload.contains("1234567890"));
        assert!(audit_payload.contains("7890"));
    }

    #[tokio::test]
    async fn create_requires_prior_verified_inquiry_and_does_not_requery_provider() {
        let store_id = Uuid::new_v4();
        let repository = Arc::new(MockStoreBankRepository::default());
        repository.store_profile.lock().unwrap().replace(StoreBankStoreProfile {
            store_id,
            owner_user_id: Uuid::new_v4(),
            store_name: "Alpha".into(),
        });
        let provider = Arc::new(MockProvider::default());
        let service = StoreBankService::new(
            repository,
            Arc::new(MockInquiryCache::default()),
            provider.clone(),
            Arc::new(MockAuditRepository::default()),
        );
        let actor = owner_actor(store_id);

        let error = service
            .create_account(
                store_id,
                CreateStoreBankAccountRequest {
                    bank_code: "014".into(),
                    account_number: "1234567890".into(),
                    is_default: true,
                },
                &actor,
            )
            .await
            .unwrap_err();

        assert!(matches!(error, AppError::BadRequest(_)));
        assert_eq!(*provider.inquiry_call_count.lock().unwrap(), 0);
    }
}
