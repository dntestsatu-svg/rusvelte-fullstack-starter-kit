use async_trait::async_trait;
use uuid::Uuid;
use super::session::Session;
use super::user::AuthUser;

#[async_trait]
pub trait AuthRepository: Send + Sync {
    async fn find_user_by_email(&self, email: &str) -> anyhow::Result<Option<AuthUser>>;
    async fn find_user_by_id(&self, id: Uuid) -> anyhow::Result<Option<AuthUser>>;
    async fn create_session(&self, session: &Session) -> anyhow::Result<()>;
    async fn find_session_by_id(&self, id: Uuid) -> anyhow::Result<Option<Session>>;
    async fn delete_session(&self, id: Uuid) -> anyhow::Result<()>;
    async fn delete_expired_sessions(&self) -> anyhow::Result<()>;
}
