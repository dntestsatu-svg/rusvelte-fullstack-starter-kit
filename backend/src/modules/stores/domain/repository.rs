use anyhow::Result;
use async_trait::async_trait;
use uuid::Uuid;

use crate::modules::stores::domain::entity::{
    Store, StoreMember, StoreMemberDetail, StoreStatus, StoreSummary,
};

#[async_trait]
pub trait StoreRepository: Send + Sync {
    async fn list_stores(
        &self,
        limit: i64,
        offset: i64,
        search: Option<&str>,
        status: Option<StoreStatus>,
        user_scope: Option<Uuid>,
    ) -> Result<Vec<StoreSummary>>;
    async fn count_stores(
        &self,
        search: Option<&str>,
        status: Option<StoreStatus>,
        user_scope: Option<Uuid>,
    ) -> Result<i64>;
    async fn find_store_by_id(&self, id: Uuid) -> Result<Option<Store>>;
    async fn find_store_summary_by_id(&self, id: Uuid) -> Result<Option<StoreSummary>>;
    async fn find_store_by_slug(&self, slug: &str) -> Result<Option<Store>>;
    async fn create_store(
        &self,
        store: Store,
        owner_member: StoreMember,
        creator_member: Option<StoreMember>,
    ) -> Result<Store>;
    async fn update_store(&self, store: Store) -> Result<Store>;
    async fn list_members(&self, store_id: Uuid) -> Result<Vec<StoreMemberDetail>>;
    async fn find_member_by_id(
        &self,
        store_id: Uuid,
        member_id: Uuid,
    ) -> Result<Option<StoreMember>>;
    async fn find_member_by_user_id(
        &self,
        store_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<StoreMember>>;
    async fn upsert_member(&self, member: StoreMember) -> Result<StoreMemberDetail>;
    async fn update_member(&self, member: StoreMember) -> Result<StoreMemberDetail>;
    async fn deactivate_member(&self, store_id: Uuid, member_id: Uuid) -> Result<()>;
}
