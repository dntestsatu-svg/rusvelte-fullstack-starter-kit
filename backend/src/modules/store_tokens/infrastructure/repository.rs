use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::modules::store_tokens::domain::entity::{NewStoreApiTokenRecord, StoreApiTokenRecord};
use crate::modules::store_tokens::domain::repository::StoreTokenRepository;

pub struct SqlxStoreTokenRepository {
    pool: PgPool,
}

impl SqlxStoreTokenRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl StoreTokenRepository for SqlxStoreTokenRepository {
    async fn store_exists(&self, store_id: Uuid) -> Result<bool> {
        let exists = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS (
                SELECT 1
                FROM stores
                WHERE id = $1
                  AND deleted_at IS NULL
            )
            "#,
        )
        .bind(store_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(exists)
    }

    async fn list_active_tokens(&self, store_id: Uuid) -> Result<Vec<StoreApiTokenRecord>> {
        sqlx::query_as::<_, StoreApiTokenRecord>(
            r#"
            SELECT
                id,
                store_id,
                name,
                token_prefix,
                token_hash,
                last_used_at,
                expires_at,
                revoked_at,
                created_by,
                created_at
            FROM store_api_tokens
            WHERE store_id = $1
              AND revoked_at IS NULL
              AND (expires_at IS NULL OR expires_at > now())
            ORDER BY created_at DESC
            "#,
        )
        .bind(store_id)
        .fetch_all(&self.pool)
        .await
        .map_err(Into::into)
    }

    async fn insert_token(&self, token: NewStoreApiTokenRecord) -> Result<StoreApiTokenRecord> {
        sqlx::query_as::<_, StoreApiTokenRecord>(
            r#"
            INSERT INTO store_api_tokens (
                id,
                store_id,
                name,
                token_prefix,
                token_hash,
                expires_at,
                created_by,
                created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING
                id,
                store_id,
                name,
                token_prefix,
                token_hash,
                last_used_at,
                expires_at,
                revoked_at,
                created_by,
                created_at
            "#,
        )
        .bind(token.id)
        .bind(token.store_id)
        .bind(token.name)
        .bind(token.token_prefix)
        .bind(token.token_hash)
        .bind(token.expires_at)
        .bind(token.created_by)
        .bind(token.created_at)
        .fetch_one(&self.pool)
        .await
        .map_err(Into::into)
    }

    async fn find_token_by_lookup_prefix(
        &self,
        token_prefix: &str,
    ) -> Result<Option<StoreApiTokenRecord>> {
        sqlx::query_as::<_, StoreApiTokenRecord>(
            r#"
            SELECT
                id,
                store_id,
                name,
                token_prefix,
                token_hash,
                last_used_at,
                expires_at,
                revoked_at,
                created_by,
                created_at
            FROM store_api_tokens
            WHERE token_prefix = $1
            LIMIT 1
            "#,
        )
        .bind(token_prefix)
        .fetch_optional(&self.pool)
        .await
        .map_err(Into::into)
    }

    async fn find_token_by_id(
        &self,
        store_id: Uuid,
        token_id: Uuid,
    ) -> Result<Option<StoreApiTokenRecord>> {
        sqlx::query_as::<_, StoreApiTokenRecord>(
            r#"
            SELECT
                id,
                store_id,
                name,
                token_prefix,
                token_hash,
                last_used_at,
                expires_at,
                revoked_at,
                created_by,
                created_at
            FROM store_api_tokens
            WHERE store_id = $1
              AND id = $2
            LIMIT 1
            "#,
        )
        .bind(store_id)
        .bind(token_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(Into::into)
    }

    async fn mark_token_revoked(
        &self,
        store_id: Uuid,
        token_id: Uuid,
        revoked_at: DateTime<Utc>,
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE store_api_tokens
            SET revoked_at = $1
            WHERE store_id = $2
              AND id = $3
              AND revoked_at IS NULL
            "#,
        )
        .bind(revoked_at)
        .bind(store_id)
        .bind(token_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    async fn touch_last_used_at(&self, token_id: Uuid, last_used_at: DateTime<Utc>) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE store_api_tokens
            SET last_used_at = $1
            WHERE id = $2
            "#,
        )
        .bind(last_used_at)
        .bind(token_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
