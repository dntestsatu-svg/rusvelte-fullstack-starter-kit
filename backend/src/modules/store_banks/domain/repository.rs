use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::modules::store_banks::domain::entity::{
    NewStoreBankAccountRecord, StoreBankAccountSecret, StoreBankAccountSummary,
    StoreBankInquiry, StoreBankStoreProfile,
};

#[async_trait]
pub trait StoreBankRepository: Send + Sync {
    async fn find_store_profile(
        &self,
        store_id: Uuid,
    ) -> anyhow::Result<Option<StoreBankStoreProfile>>;

    async fn list_by_store(&self, store_id: Uuid) -> anyhow::Result<Vec<StoreBankAccountSummary>>;

    async fn insert_verified_account(
        &self,
        record: NewStoreBankAccountRecord,
    ) -> anyhow::Result<StoreBankAccountSummary>;

    async fn set_default_bank(
        &self,
        store_id: Uuid,
        bank_account_id: Uuid,
        updated_at: DateTime<Utc>,
    ) -> anyhow::Result<Option<StoreBankAccountSummary>>;

    async fn find_account_with_secret(
        &self,
        store_id: Uuid,
        bank_account_id: Uuid,
    ) -> anyhow::Result<Option<StoreBankAccountSecret>>;

    async fn find_default_account_with_secret(
        &self,
        store_id: Uuid,
    ) -> anyhow::Result<Option<StoreBankAccountSecret>>;
}

#[async_trait]
pub trait StoreBankInquiryCache: Send + Sync {
    async fn remember_verified_inquiry(
        &self,
        actor_user_id: Uuid,
        store_id: Uuid,
        bank_code: &str,
        account_number: &str,
        inquiry: &StoreBankInquiry,
    ) -> anyhow::Result<()>;

    async fn find_verified_inquiry(
        &self,
        actor_user_id: Uuid,
        store_id: Uuid,
        bank_code: &str,
        account_number: &str,
    ) -> anyhow::Result<Option<StoreBankInquiry>>;
}
