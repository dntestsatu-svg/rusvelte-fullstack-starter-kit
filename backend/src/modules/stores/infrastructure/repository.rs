use anyhow::Result;
use async_trait::async_trait;
use sqlx::PgPool;
use std::str::FromStr;
use uuid::Uuid;

use crate::modules::stores::domain::entity::{
    Store, StoreMember, StoreMemberDetail, StoreMemberStatus, StoreStatus, StoreSummary,
};
use crate::modules::stores::domain::repository::StoreRepository;
use crate::shared::auth::StoreRole;

pub struct SqlxStoreRepository {
    pool: PgPool,
}

impl SqlxStoreRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    async fn find_member_detail_by_id(&self, member_id: Uuid) -> Result<Option<StoreMemberDetail>> {
        let row = sqlx::query!(
            r#"
            SELECT
                sm.id,
                sm.store_id,
                sm.user_id,
                u.name AS user_name,
                u.email AS user_email,
                u.role AS user_platform_role,
                sm.store_role,
                sm.status,
                sm.invited_by,
                sm.created_at,
                sm.updated_at
            FROM store_members sm
            INNER JOIN users u ON u.id = sm.user_id
            INNER JOIN stores s ON s.id = sm.store_id
            WHERE sm.id = $1
              AND s.deleted_at IS NULL
              AND u.deleted_at IS NULL
            "#,
            member_id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|record| StoreMemberDetail {
            id: record.id,
            store_id: record.store_id,
            user_id: record.user_id,
            user_name: record.user_name,
            user_email: record.user_email,
            user_platform_role: record.user_platform_role,
            store_role: StoreRole::from_str(&record.store_role).unwrap_or(StoreRole::Viewer),
            status: StoreMemberStatus::from(record.status),
            invited_by: record.invited_by,
            created_at: record.created_at,
            updated_at: record.updated_at,
        }))
    }
}

#[async_trait]
impl StoreRepository for SqlxStoreRepository {
    async fn list_stores(
        &self,
        limit: i64,
        offset: i64,
        search: Option<&str>,
        status: Option<StoreStatus>,
        user_scope: Option<Uuid>,
    ) -> Result<Vec<StoreSummary>> {
        let search = search.map(str::trim).filter(|value| !value.is_empty());
        let status = status.map(|value| value.to_string());

        let rows = sqlx::query!(
            r#"
            SELECT
                s.id,
                s.owner_user_id,
                owner.name AS owner_name,
                owner.email AS owner_email,
                s.name,
                s.slug,
                s.status,
                s.callback_url,
                s.provider_username,
                s.created_at,
                s.updated_at
            FROM stores s
            INNER JOIN users owner ON owner.id = s.owner_user_id
            WHERE s.deleted_at IS NULL
              AND owner.deleted_at IS NULL
              AND (
                $1::UUID IS NULL
                OR EXISTS (
                    SELECT 1
                    FROM store_members sm
                    WHERE sm.store_id = s.id
                      AND sm.user_id = $1
                      AND sm.status = 'active'
                )
              )
              AND ($2::TEXT IS NULL OR s.status = $2)
              AND (
                $3::TEXT IS NULL
                OR s.name ILIKE '%' || $3 || '%'
                OR s.slug ILIKE '%' || $3 || '%'
                OR owner.name ILIKE '%' || $3 || '%'
                OR owner.email ILIKE '%' || $3 || '%'
              )
            ORDER BY s.created_at DESC
            LIMIT $4 OFFSET $5
            "#,
            user_scope,
            status,
            search,
            limit,
            offset
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|record| StoreSummary {
                id: record.id,
                owner_user_id: record.owner_user_id,
                owner_name: record.owner_name,
                owner_email: record.owner_email,
                name: record.name,
                slug: record.slug,
                status: StoreStatus::from(record.status),
                callback_url: record.callback_url,
                provider_username: record.provider_username.unwrap_or_default(),
                created_at: record.created_at,
                updated_at: record.updated_at,
            })
            .collect())
    }

    async fn count_stores(
        &self,
        search: Option<&str>,
        status: Option<StoreStatus>,
        user_scope: Option<Uuid>,
    ) -> Result<i64> {
        let search = search.map(str::trim).filter(|value| !value.is_empty());
        let status = status.map(|value| value.to_string());

        let count = sqlx::query_scalar!(
            r#"
            SELECT COUNT(s.id)
            FROM stores s
            INNER JOIN users owner ON owner.id = s.owner_user_id
            WHERE s.deleted_at IS NULL
              AND owner.deleted_at IS NULL
              AND (
                $1::UUID IS NULL
                OR EXISTS (
                    SELECT 1
                    FROM store_members sm
                    WHERE sm.store_id = s.id
                      AND sm.user_id = $1
                      AND sm.status = 'active'
                )
              )
              AND ($2::TEXT IS NULL OR s.status = $2)
              AND (
                $3::TEXT IS NULL
                OR s.name ILIKE '%' || $3 || '%'
                OR s.slug ILIKE '%' || $3 || '%'
                OR owner.name ILIKE '%' || $3 || '%'
                OR owner.email ILIKE '%' || $3 || '%'
              )
            "#,
            user_scope,
            status,
            search
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(count.unwrap_or(0))
    }

    async fn find_store_by_id(&self, id: Uuid) -> Result<Option<Store>> {
        let row = sqlx::query!(
            r#"
            SELECT
                id,
                owner_user_id,
                name,
                slug,
                status,
                callback_url,
                callback_secret,
                provider_username,
                created_at,
                updated_at,
                deleted_at
            FROM stores
            WHERE id = $1 AND deleted_at IS NULL
            "#,
            id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|record| Store {
            id: record.id,
            owner_user_id: record.owner_user_id,
            name: record.name,
            slug: record.slug,
            status: StoreStatus::from(record.status),
            callback_url: record.callback_url,
            callback_secret: record.callback_secret,
            provider_username: record.provider_username.unwrap_or_default(),
            created_at: record.created_at,
            updated_at: record.updated_at,
            deleted_at: record.deleted_at,
        }))
    }

    async fn find_store_summary_by_id(&self, id: Uuid) -> Result<Option<StoreSummary>> {
        let row = sqlx::query!(
            r#"
            SELECT
                s.id,
                s.owner_user_id,
                owner.name AS owner_name,
                owner.email AS owner_email,
                s.name,
                s.slug,
                s.status,
                s.callback_url,
                s.provider_username,
                s.created_at,
                s.updated_at
            FROM stores s
            INNER JOIN users owner ON owner.id = s.owner_user_id
            WHERE s.id = $1
              AND s.deleted_at IS NULL
              AND owner.deleted_at IS NULL
            "#,
            id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|record| StoreSummary {
            id: record.id,
            owner_user_id: record.owner_user_id,
            owner_name: record.owner_name,
            owner_email: record.owner_email,
            name: record.name,
            slug: record.slug,
            status: StoreStatus::from(record.status),
            callback_url: record.callback_url,
            provider_username: record.provider_username.unwrap_or_default(),
            created_at: record.created_at,
            updated_at: record.updated_at,
        }))
    }

    async fn find_store_by_slug(&self, slug: &str) -> Result<Option<Store>> {
        let row = sqlx::query!(
            r#"
            SELECT
                id,
                owner_user_id,
                name,
                slug,
                status,
                callback_url,
                callback_secret,
                provider_username,
                created_at,
                updated_at,
                deleted_at
            FROM stores
            WHERE slug = $1 AND deleted_at IS NULL
            "#,
            slug
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|record| Store {
            id: record.id,
            owner_user_id: record.owner_user_id,
            name: record.name,
            slug: record.slug,
            status: StoreStatus::from(record.status),
            callback_url: record.callback_url,
            callback_secret: record.callback_secret,
            provider_username: record.provider_username.unwrap_or_default(),
            created_at: record.created_at,
            updated_at: record.updated_at,
            deleted_at: record.deleted_at,
        }))
    }

    async fn create_store(
        &self,
        store: Store,
        owner_member: StoreMember,
        creator_member: Option<StoreMember>,
    ) -> Result<Store> {
        let mut transaction = self.pool.begin().await?;

        sqlx::query!(
            r#"
            INSERT INTO stores (
                id,
                owner_user_id,
                name,
                slug,
                status,
                callback_url,
                callback_secret,
                provider_username
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
            store.id,
            store.owner_user_id,
            store.name,
            store.slug,
            store.status.to_string(),
            store.callback_url,
            store.callback_secret,
            store.provider_username
        )
        .execute(&mut *transaction)
        .await?;

        sqlx::query!(
            r#"
            INSERT INTO store_members (
                id,
                store_id,
                user_id,
                store_role,
                status,
                invited_by,
                created_at,
                updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
            owner_member.id,
            owner_member.store_id,
            owner_member.user_id,
            owner_member.store_role.to_string(),
            owner_member.status.to_string(),
            owner_member.invited_by,
            owner_member.created_at,
            owner_member.updated_at
        )
        .execute(&mut *transaction)
        .await?;

        if let Some(member) = creator_member {
            sqlx::query!(
                r#"
                INSERT INTO store_members (
                    id,
                    store_id,
                    user_id,
                    store_role,
                    status,
                    invited_by,
                    created_at,
                    updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                ON CONFLICT (store_id, user_id)
                DO UPDATE SET
                    store_role = EXCLUDED.store_role,
                    status = EXCLUDED.status,
                    invited_by = EXCLUDED.invited_by,
                    updated_at = EXCLUDED.updated_at
                "#,
                member.id,
                member.store_id,
                member.user_id,
                member.store_role.to_string(),
                member.status.to_string(),
                member.invited_by,
                member.created_at,
                member.updated_at
            )
            .execute(&mut *transaction)
            .await?;
        }

        transaction.commit().await?;
        Ok(store)
    }

    async fn update_store(&self, store: Store) -> Result<Store> {
        sqlx::query!(
            r#"
            UPDATE stores
            SET
                owner_user_id = $1,
                name = $2,
                slug = $3,
                status = $4,
                callback_url = $5,
                provider_username = $6,
                updated_at = now()
            WHERE id = $7
              AND deleted_at IS NULL
            "#,
            store.owner_user_id,
            store.name,
            store.slug,
            store.status.to_string(),
            store.callback_url,
            store.provider_username,
            store.id
        )
        .execute(&self.pool)
        .await?;

        Ok(store)
    }

    async fn list_members(&self, store_id: Uuid) -> Result<Vec<StoreMemberDetail>> {
        let rows = sqlx::query!(
            r#"
            SELECT
                sm.id,
                sm.store_id,
                sm.user_id,
                u.name AS user_name,
                u.email AS user_email,
                u.role AS user_platform_role,
                sm.store_role,
                sm.status,
                sm.invited_by,
                sm.created_at,
                sm.updated_at
            FROM store_members sm
            INNER JOIN users u ON u.id = sm.user_id
            INNER JOIN stores s ON s.id = sm.store_id
            WHERE sm.store_id = $1
              AND s.deleted_at IS NULL
              AND u.deleted_at IS NULL
            ORDER BY sm.created_at ASC
            "#,
            store_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|record| StoreMemberDetail {
                id: record.id,
                store_id: record.store_id,
                user_id: record.user_id,
                user_name: record.user_name,
                user_email: record.user_email,
                user_platform_role: record.user_platform_role,
                store_role: StoreRole::from_str(&record.store_role).unwrap_or(StoreRole::Viewer),
                status: StoreMemberStatus::from(record.status),
                invited_by: record.invited_by,
                created_at: record.created_at,
                updated_at: record.updated_at,
            })
            .collect())
    }

    async fn find_member_by_id(
        &self,
        store_id: Uuid,
        member_id: Uuid,
    ) -> Result<Option<StoreMember>> {
        let row = sqlx::query!(
            r#"
            SELECT id, store_id, user_id, store_role, status, invited_by, created_at, updated_at
            FROM store_members
            WHERE store_id = $1 AND id = $2
            "#,
            store_id,
            member_id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|record| StoreMember {
            id: record.id,
            store_id: record.store_id,
            user_id: record.user_id,
            store_role: StoreRole::from_str(&record.store_role).unwrap_or(StoreRole::Viewer),
            status: StoreMemberStatus::from(record.status),
            invited_by: record.invited_by,
            created_at: record.created_at,
            updated_at: record.updated_at,
        }))
    }

    async fn find_member_by_user_id(
        &self,
        store_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<StoreMember>> {
        let row = sqlx::query!(
            r#"
            SELECT id, store_id, user_id, store_role, status, invited_by, created_at, updated_at
            FROM store_members
            WHERE store_id = $1 AND user_id = $2
            "#,
            store_id,
            user_id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|record| StoreMember {
            id: record.id,
            store_id: record.store_id,
            user_id: record.user_id,
            store_role: StoreRole::from_str(&record.store_role).unwrap_or(StoreRole::Viewer),
            status: StoreMemberStatus::from(record.status),
            invited_by: record.invited_by,
            created_at: record.created_at,
            updated_at: record.updated_at,
        }))
    }

    async fn upsert_member(&self, member: StoreMember) -> Result<StoreMemberDetail> {
        let member_id = sqlx::query_scalar!(
            r#"
            INSERT INTO store_members (
                id,
                store_id,
                user_id,
                store_role,
                status,
                invited_by,
                created_at,
                updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (store_id, user_id)
            DO UPDATE SET
                store_role = EXCLUDED.store_role,
                status = EXCLUDED.status,
                invited_by = EXCLUDED.invited_by,
                updated_at = EXCLUDED.updated_at
            RETURNING id
            "#,
            member.id,
            member.store_id,
            member.user_id,
            member.store_role.to_string(),
            member.status.to_string(),
            member.invited_by,
            member.created_at,
            member.updated_at
        )
        .fetch_one(&self.pool)
        .await?;

        self.find_member_detail_by_id(member_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Member not found after upsert"))
    }

    async fn update_member(&self, member: StoreMember) -> Result<StoreMemberDetail> {
        sqlx::query!(
            r#"
            UPDATE store_members
            SET
                store_role = $1,
                status = $2,
                invited_by = $3,
                updated_at = now()
            WHERE id = $4
              AND store_id = $5
            "#,
            member.store_role.to_string(),
            member.status.to_string(),
            member.invited_by,
            member.id,
            member.store_id
        )
        .execute(&self.pool)
        .await?;

        self.find_member_detail_by_id(member.id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Member not found after update"))
    }

    async fn deactivate_member(&self, store_id: Uuid, member_id: Uuid) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE store_members
            SET status = 'inactive', updated_at = now()
            WHERE store_id = $1 AND id = $2
            "#,
            store_id,
            member_id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
