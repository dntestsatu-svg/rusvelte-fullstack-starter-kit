use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::modules::store_tokens::domain::entity::{NewStoreApiTokenRecord, StoreApiTokenRecord};

#[async_trait]
pub trait StoreTokenRepository: Send + Sync {
    async fn store_exists(&self, store_id: Uuid) -> Result<bool>;
    async fn list_active_tokens(&self, store_id: Uuid) -> Result<Vec<StoreApiTokenRecord>>;
    async fn insert_token(&self, token: NewStoreApiTokenRecord) -> Result<StoreApiTokenRecord>;
    async fn find_token_by_lookup_prefix(
        &self,
        token_prefix: &str,
    ) -> Result<Option<StoreApiTokenRecord>>;
    async fn find_token_by_id(
        &self,
        store_id: Uuid,
        token_id: Uuid,
    ) -> Result<Option<StoreApiTokenRecord>>;
    async fn mark_token_revoked(
        &self,
        store_id: Uuid,
        token_id: Uuid,
        revoked_at: DateTime<Utc>,
    ) -> Result<bool>;
    async fn touch_last_used_at(&self, token_id: Uuid, last_used_at: DateTime<Utc>) -> Result<()>;
}
