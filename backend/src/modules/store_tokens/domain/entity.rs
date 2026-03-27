use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

pub const STORE_API_TOKEN_PREFIX: &str = "jq_sk_";
pub const STORE_API_TOKEN_LOOKUP_LENGTH: usize = 12;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct StoreApiTokenRecord {
    pub id: Uuid,
    pub store_id: Uuid,
    pub name: String,
    pub token_prefix: String,
    pub token_hash: String,
    pub last_used_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewStoreApiTokenRecord {
    pub id: Uuid,
    pub store_id: Uuid,
    pub name: String,
    pub token_prefix: String,
    pub token_hash: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoreApiTokenMetadata {
    pub id: Uuid,
    pub name: String,
    pub display_prefix: String,
    pub last_used_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoreApiTokenAuthContext {
    pub store_id: Uuid,
    pub token_id: Uuid,
    pub token_identifier: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateStoreApiTokenResult {
    pub plaintext_token: String,
    pub token: StoreApiTokenMetadata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevokeStoreApiTokenResult {
    Revoked,
    AlreadyRevoked,
}

pub fn masked_display_prefix(token_prefix: &str) -> String {
    let lookup = token_prefix
        .strip_prefix(STORE_API_TOKEN_PREFIX)
        .unwrap_or(token_prefix);
    let suffix_start = lookup.len().saturating_sub(4);
    let suffix = &lookup[suffix_start..];

    format!("{STORE_API_TOKEN_PREFIX}****{suffix}")
}

pub fn is_store_api_token_expired(expires_at: Option<DateTime<Utc>>, now: DateTime<Utc>) -> bool {
    expires_at.map(|value| value <= now).unwrap_or(false)
}
