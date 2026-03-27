use crate::modules::users::domain::entity::{User, UserStatus};
use crate::shared::auth::{PlatformRole, StoreRole};
use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use uuid::Uuid;

#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn find_by_id(&self, id: Uuid) -> Result<Option<User>>;
    async fn find_by_email(&self, email: &str) -> Result<Option<User>>;
    async fn list_memberships(&self, user_id: Uuid) -> Result<HashMap<Uuid, StoreRole>>;
    async fn user_exists_in_scope(&self, actor_user_id: Uuid, target_user_id: Uuid)
        -> Result<bool>;
    async fn list_users(
        &self,
        limit: i64,
        offset: i64,
        role_filter: Option<PlatformRole>,
        search: Option<&str>,
        store_scope: Option<Uuid>, // Used for Admin scoping
    ) -> Result<Vec<User>>;
    async fn count_users(
        &self,
        role_filter: Option<PlatformRole>,
        search: Option<&str>,
        store_scope: Option<Uuid>,
    ) -> Result<i64>;
    async fn create(&self, user: User) -> Result<User>;
    async fn update(&self, user: User) -> Result<User>;
    async fn update_status(&self, id: Uuid, status: UserStatus) -> Result<()>;
}
