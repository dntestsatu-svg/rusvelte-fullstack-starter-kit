use async_trait::async_trait;
use uuid::Uuid;

use crate::modules::payouts::domain::entity::{
    NewPayoutRecord, PayoutListRow, PayoutRecord, UpdatePayoutStatus,
};

#[async_trait]
pub trait PayoutRepository: Send + Sync {
    async fn insert_payout(&self, record: NewPayoutRecord) -> anyhow::Result<PayoutRecord>;

    async fn update_status(&self, update: UpdatePayoutStatus) -> anyhow::Result<Option<PayoutRecord>>;

    async fn find_by_id(&self, store_id: Uuid, payout_id: Uuid) -> anyhow::Result<Option<PayoutRecord>>;

    async fn list_by_store(
        &self,
        store_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> anyhow::Result<Vec<PayoutListRow>>;
}
