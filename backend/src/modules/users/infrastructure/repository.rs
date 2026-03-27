use crate::modules::users::domain::entity::{User, UserStatus};
use crate::modules::users::domain::repository::UserRepository;
use crate::shared::auth::{PlatformRole, StoreRole};
use anyhow::Result;
use async_trait::async_trait;
use sqlx::PgPool;
use std::collections::HashMap;
use std::str::FromStr;
use uuid::Uuid;

pub struct SqlxUserRepository {
    pool: PgPool,
}

impl SqlxUserRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UserRepository for SqlxUserRepository {
    async fn find_by_id(&self, id: Uuid) -> Result<Option<User>> {
        let row = sqlx::query!(
            r#"
            SELECT id, name, email, password_hash, role, status, created_by, last_login_at, created_at, updated_at, deleted_at
            FROM users
            WHERE id = $1 AND deleted_at IS NULL
            "#,
            id
        )
        .fetch_optional(&self.pool)
        .await?;

        let row: Option<_> = row;

        Ok(row.map(|r| User {
            id: r.id,
            name: r.name,
            email: r.email,
            password_hash: r.password_hash,
            role: PlatformRole::from_str(&r.role).unwrap_or(PlatformRole::User),
            status: UserStatus::from(r.status),
            created_by: r.created_by,
            last_login_at: r.last_login_at,
            created_at: r.created_at,
            updated_at: r.updated_at,
            deleted_at: r.deleted_at,
        }))
    }

    async fn find_by_email(&self, email: &str) -> Result<Option<User>> {
        let row = sqlx::query!(
            r#"
            SELECT id, name, email, password_hash, role, status, created_by, last_login_at, created_at, updated_at, deleted_at
            FROM users
            WHERE LOWER(email) = LOWER($1) AND deleted_at IS NULL
            "#,
            email
        )
        .fetch_optional(&self.pool)
        .await?;

        let row: Option<_> = row;

        Ok(row.map(|r| User {
            id: r.id,
            name: r.name,
            email: r.email,
            password_hash: r.password_hash,
            role: PlatformRole::from_str(&r.role).unwrap_or(PlatformRole::User),
            status: UserStatus::from(r.status),
            created_by: r.created_by,
            last_login_at: r.last_login_at,
            created_at: r.created_at,
            updated_at: r.updated_at,
            deleted_at: r.deleted_at,
        }))
    }

    async fn list_memberships(&self, user_id: Uuid) -> Result<HashMap<Uuid, StoreRole>> {
        let rows = sqlx::query!(
            r#"
            SELECT sm.store_id, sm.store_role
            FROM store_members sm
            INNER JOIN stores s ON s.id = sm.store_id
            WHERE sm.user_id = $1
              AND sm.status = 'active'
              AND s.deleted_at IS NULL
            "#,
            user_id
        )
        .fetch_all(&self.pool)
        .await?;

        let memberships = rows
            .into_iter()
            .filter_map(|row| {
                StoreRole::from_str(&row.store_role)
                    .ok()
                    .map(|role| (row.store_id, role))
            })
            .collect();

        Ok(memberships)
    }

    async fn user_exists_in_scope(
        &self,
        actor_user_id: Uuid,
        target_user_id: Uuid,
    ) -> Result<bool> {
        let is_visible = sqlx::query_scalar!(
            r#"
            SELECT EXISTS(
                SELECT 1
                FROM users u
                WHERE u.id = $2
                  AND u.deleted_at IS NULL
                  AND (
                    u.id = $1
                    OR EXISTS (
                        SELECT 1
                        FROM store_members actor_sm
                        INNER JOIN store_members target_sm
                            ON target_sm.store_id = actor_sm.store_id
                        INNER JOIN stores s
                            ON s.id = actor_sm.store_id
                        WHERE actor_sm.user_id = $1
                          AND actor_sm.status = 'active'
                          AND target_sm.user_id = u.id
                          AND target_sm.status = 'active'
                          AND s.deleted_at IS NULL
                    )
                  )
            )
            "#,
            actor_user_id,
            target_user_id
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(is_visible.unwrap_or(false))
    }

    async fn list_users(
        &self,
        limit: i64,
        offset: i64,
        role_filter: Option<PlatformRole>,
        search: Option<&str>,
        store_scope: Option<Uuid>,
    ) -> Result<Vec<User>> {
        let role_str = role_filter.map(|r| r.to_string());
        let search = search.map(str::trim).filter(|value| !value.is_empty());

        let rows = sqlx::query!(
            r#"
            SELECT u.id, u.name, u.email, u.password_hash, u.role, u.status, u.created_by, u.last_login_at, u.created_at, u.updated_at, u.deleted_at
            FROM users u
            WHERE u.deleted_at IS NULL
              AND (
                $1::UUID IS NULL
                OR u.id = $1
                OR EXISTS (
                    SELECT 1
                    FROM store_members actor_sm
                    INNER JOIN store_members target_sm
                        ON target_sm.store_id = actor_sm.store_id
                    INNER JOIN stores s
                        ON s.id = actor_sm.store_id
                    WHERE actor_sm.user_id = $1
                      AND actor_sm.status = 'active'
                      AND target_sm.user_id = u.id
                      AND target_sm.status = 'active'
                      AND s.deleted_at IS NULL
                )
              )
              AND ($2::TEXT IS NULL OR u.role = $2)
              AND (
                $3::TEXT IS NULL
                OR u.name ILIKE '%' || $3 || '%'
                OR u.email ILIKE '%' || $3 || '%'
              )
            ORDER BY u.created_at DESC
            LIMIT $4 OFFSET $5
            "#,
            store_scope,
            role_str,
            search,
            limit,
            offset
        )
        .fetch_all(&self.pool)
        .await?;

        let rows: Vec<_> = rows;

        Ok(rows
            .into_iter()
            .map(|r| User {
                id: r.id,
                name: r.name,
                email: r.email,
                password_hash: r.password_hash,
                role: PlatformRole::from_str(&r.role).unwrap_or(PlatformRole::User),
                status: UserStatus::from(r.status),
                created_by: r.created_by,
                last_login_at: r.last_login_at,
                created_at: r.created_at,
                updated_at: r.updated_at,
                deleted_at: r.deleted_at,
            })
            .collect())
    }

    async fn count_users(
        &self,
        role_filter: Option<PlatformRole>,
        search: Option<&str>,
        store_scope: Option<Uuid>,
    ) -> Result<i64> {
        let role_str = role_filter.map(|r| r.to_string());
        let search = search.map(str::trim).filter(|value| !value.is_empty());

        let count = sqlx::query_scalar!(
            r#"
            SELECT COUNT(u.id)
            FROM users u
            WHERE u.deleted_at IS NULL
              AND (
                $1::UUID IS NULL
                OR u.id = $1
                OR EXISTS (
                    SELECT 1
                    FROM store_members actor_sm
                    INNER JOIN store_members target_sm
                        ON target_sm.store_id = actor_sm.store_id
                    INNER JOIN stores s
                        ON s.id = actor_sm.store_id
                    WHERE actor_sm.user_id = $1
                      AND actor_sm.status = 'active'
                      AND target_sm.user_id = u.id
                      AND target_sm.status = 'active'
                      AND s.deleted_at IS NULL
                )
              )
              AND ($2::TEXT IS NULL OR u.role = $2)
              AND (
                $3::TEXT IS NULL
                OR u.name ILIKE '%' || $3 || '%'
                OR u.email ILIKE '%' || $3 || '%'
              )
            "#,
            store_scope,
            role_str,
            search
        )
        .fetch_one(&self.pool)
        .await?;

        let count: Option<i64> = count;

        Ok(count.unwrap_or(0))
    }

    async fn create(&self, user: User) -> Result<User> {
        let role_str = user.role.to_string();
        let status_str = user.status.to_string();

        sqlx::query!(
            r#"
            INSERT INTO users (id, name, email, password_hash, role, status, created_by)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
            user.id,
            user.name,
            user.email.to_lowercase(),
            user.password_hash,
            role_str,
            status_str,
            user.created_by
        )
        .execute(&self.pool)
        .await?;

        Ok(user)
    }

    async fn update(&self, user: User) -> Result<User> {
        let role_str = user.role.to_string();
        let status_str = user.status.to_string();

        sqlx::query!(
            r#"
            UPDATE users
            SET name = $1, email = $2, role = $3, status = $4, updated_at = now()
            WHERE id = $5 AND deleted_at IS NULL
            "#,
            user.name,
            user.email.to_lowercase(),
            role_str,
            status_str,
            user.id
        )
        .execute(&self.pool)
        .await?;

        Ok(user)
    }

    async fn update_status(&self, id: Uuid, status: UserStatus) -> Result<()> {
        let status_str = status.to_string();

        sqlx::query!(
            r#"
            UPDATE users
            SET status = $1, updated_at = now()
            WHERE id = $2 AND deleted_at IS NULL
            "#,
            status_str,
            id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
