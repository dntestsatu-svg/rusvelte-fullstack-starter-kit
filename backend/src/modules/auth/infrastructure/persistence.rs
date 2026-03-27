use crate::modules::auth::domain::repository::AuthRepository;
use crate::modules::auth::domain::session::Session;
use crate::modules::auth::domain::user::AuthUser;
use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

pub struct PostgresAuthRepository {
    pool: PgPool,
}

impl PostgresAuthRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AuthRepository for PostgresAuthRepository {
    async fn find_user_by_email(&self, email: &str) -> anyhow::Result<Option<AuthUser>> {
        let user = sqlx::query_as!(
            AuthUser,
            r#"SELECT id, name, email, password_hash, role, status FROM users WHERE LOWER(email) = LOWER($1) AND deleted_at IS NULL"#,
            email
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(user)
    }

    async fn find_user_by_id(&self, id: Uuid) -> anyhow::Result<Option<AuthUser>> {
        let user = sqlx::query_as!(
            AuthUser,
            r#"SELECT id, name, email, password_hash, role, status FROM users WHERE id = $1 AND deleted_at IS NULL"#,
            id
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(user)
    }

    async fn update_last_login(&self, id: Uuid) -> anyhow::Result<()> {
        sqlx::query!(
            r#"
            UPDATE users
            SET last_login_at = now(), updated_at = now()
            WHERE id = $1 AND deleted_at IS NULL
            "#,
            id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn create_session(&self, session: &Session) -> anyhow::Result<()> {
        sqlx::query(
            r#"INSERT INTO sessions (id, user_id, csrf_token, expires_at, created_at) VALUES ($1, $2, $3, $4, $5)"#
        )
        .bind(session.id)
        .bind(session.user_id)
        .bind(&session.csrf_token)
        .bind(session.expires_at)
        .bind(session.created_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn find_session_by_id(&self, id: Uuid) -> anyhow::Result<Option<Session>> {
        let row = sqlx::query_as::<_, Session>(
            r#"SELECT id, user_id, csrf_token, expires_at, created_at FROM sessions WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }

    async fn delete_session(&self, id: Uuid) -> anyhow::Result<()> {
        sqlx::query!(r#"DELETE FROM sessions WHERE id = $1"#, id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn delete_expired_sessions(&self) -> anyhow::Result<()> {
        sqlx::query!(r#"DELETE FROM sessions WHERE expires_at < NOW()"#)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
