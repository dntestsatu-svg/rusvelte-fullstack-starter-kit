use anyhow::anyhow;
use async_trait::async_trait;
use bb8_redis::redis::AsyncCommands;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::infrastructure::redis::RedisPool;
use crate::modules::store_banks::domain::entity::StoreBankInquiry;
use crate::modules::store_banks::domain::repository::StoreBankInquiryCache;

const DEFAULT_VERIFIED_INQUIRY_TTL_SECONDS: u64 = 300;

pub struct RedisStoreBankInquiryCache {
    redis: RedisPool,
    ttl_seconds: u64,
}

impl RedisStoreBankInquiryCache {
    pub fn new(redis: RedisPool) -> Self {
        Self {
            redis,
            ttl_seconds: DEFAULT_VERIFIED_INQUIRY_TTL_SECONDS,
        }
    }

    fn cache_key(
        actor_user_id: Uuid,
        store_id: Uuid,
        bank_code: &str,
        account_number: &str,
    ) -> String {
        let digest = Sha256::digest(format!("{bank_code}:{account_number}"));
        let bank_hash = digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        format!("store_bank_inquiry:{actor_user_id}:{store_id}:{bank_hash}")
    }
}

#[async_trait]
impl StoreBankInquiryCache for RedisStoreBankInquiryCache {
    async fn remember_verified_inquiry(
        &self,
        actor_user_id: Uuid,
        store_id: Uuid,
        bank_code: &str,
        account_number: &str,
        inquiry: &StoreBankInquiry,
    ) -> anyhow::Result<()> {
        let key = Self::cache_key(actor_user_id, store_id, bank_code, account_number);
        let payload = serde_json::to_string(inquiry)
            .map_err(|error| anyhow!("failed to serialize store bank inquiry cache: {error}"))?;
        let mut conn = self
            .redis
            .get()
            .await
            .map_err(|error| anyhow!("failed to get redis connection: {error}"))?;
        let _: () = conn
            .set_ex(key, payload, self.ttl_seconds)
            .await
            .map_err(|error| anyhow!("failed to store bank inquiry cache: {error}"))?;
        Ok(())
    }

    async fn find_verified_inquiry(
        &self,
        actor_user_id: Uuid,
        store_id: Uuid,
        bank_code: &str,
        account_number: &str,
    ) -> anyhow::Result<Option<StoreBankInquiry>> {
        let key = Self::cache_key(actor_user_id, store_id, bank_code, account_number);
        let mut conn = self
            .redis
            .get()
            .await
            .map_err(|error| anyhow!("failed to get redis connection: {error}"))?;
        let cached_value: Option<String> = conn
            .get(key)
            .await
            .map_err(|error| anyhow!("failed to load store bank inquiry cache: {error}"))?;
        cached_value
            .map(|value| {
                serde_json::from_str::<StoreBankInquiry>(&value).map_err(|error| {
                    anyhow!("failed to deserialize store bank inquiry cache payload: {error}")
                })
            })
            .transpose()
    }
}
