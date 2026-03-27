use std::sync::Arc;

use chrono::Utc;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::modules::stores::domain::entity::{
    Store, StoreMember, StoreMemberDetail, StoreMemberStatus, StoreStatus, StoreSummary,
};
use crate::modules::stores::domain::repository::StoreRepository;
use crate::modules::users::domain::repository::UserRepository;
use crate::shared::audit::{AuditLogEntry, AuditLogRepository};
use crate::shared::auth::{has_capability, AuthenticatedUser, Capability, PlatformRole, StoreRole};
use crate::shared::error::AppError;

#[derive(Debug, Deserialize)]
pub struct CreateStoreRequest {
    pub owner_user_id: Option<Uuid>,
    pub owner_email: Option<String>,
    pub name: String,
    pub slug: String,
    pub callback_url: Option<String>,
    pub provider_username: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateStoreRequest {
    pub owner_user_id: Option<Uuid>,
    pub owner_email: Option<String>,
    pub name: Option<String>,
    pub slug: Option<String>,
    pub status: Option<StoreStatus>,
    pub callback_url: Option<String>,
    pub provider_username: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AddStoreMemberRequest {
    pub user_id: Option<Uuid>,
    pub user_email: Option<String>,
    pub store_role: StoreRole,
}

#[derive(Debug, Deserialize)]
pub struct UpdateStoreMemberRequest {
    pub store_role: Option<StoreRole>,
    pub status: Option<StoreMemberStatus>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct StoreListFilters {
    pub search: Option<String>,
    pub status: Option<StoreStatus>,
}

pub struct StoreService {
    repository: Arc<dyn StoreRepository>,
    user_repository: Arc<dyn UserRepository>,
    audit_repository: Arc<dyn AuditLogRepository>,
}

impl StoreService {
    pub fn new(
        repository: Arc<dyn StoreRepository>,
        user_repository: Arc<dyn UserRepository>,
        audit_repository: Arc<dyn AuditLogRepository>,
    ) -> Self {
        Self {
            repository,
            user_repository,
            audit_repository,
        }
    }

    pub async fn list_stores(
        &self,
        limit: i64,
        offset: i64,
        filters: StoreListFilters,
        actor: &AuthenticatedUser,
    ) -> Result<Vec<StoreSummary>, AppError> {
        self.repository
            .list_stores(
                limit,
                offset,
                filters.search.as_deref(),
                filters.status,
                store_scope(actor),
            )
            .await
            .map_err(Into::into)
    }

    pub async fn count_stores(
        &self,
        filters: StoreListFilters,
        actor: &AuthenticatedUser,
    ) -> Result<i64, AppError> {
        self.repository
            .count_stores(
                filters.search.as_deref(),
                filters.status,
                store_scope(actor),
            )
            .await
            .map_err(Into::into)
    }

    pub async fn get_store(
        &self,
        store_id: Uuid,
        actor: &AuthenticatedUser,
    ) -> Result<Option<StoreSummary>, AppError> {
        if !has_capability(actor, Capability::StoreRead, Some(store_id)) {
            return Ok(None);
        }

        self.repository
            .find_store_summary_by_id(store_id)
            .await
            .map_err(Into::into)
    }

    pub async fn create_store(
        &self,
        req: CreateStoreRequest,
        actor: &AuthenticatedUser,
    ) -> Result<StoreSummary, AppError> {
        let owner = self
            .resolve_target_user(req.owner_user_id, req.owner_email.as_deref())
            .await?;
        let name = req.name.trim().to_string();
        let slug = normalize_slug(&req.slug);
        let callback_url = normalize_optional_string(req.callback_url);
        let provider_username = req.provider_username.trim().to_string();

        if name.is_empty() {
            return Err(AppError::BadRequest("Store name is required".into()));
        }

        if slug.is_empty() {
            return Err(AppError::BadRequest("Store slug is required".into()));
        }

        if provider_username.is_empty() {
            return Err(AppError::BadRequest("Provider username is required".into()));
        }

        validate_callback_url(callback_url.as_deref())?;
        validate_store_owner_role(&owner.role)?;

        if self.repository.find_store_by_slug(&slug).await?.is_some() {
            return Err(AppError::Conflict("Store slug is already in use".into()));
        }

        let now = Utc::now();
        let store_id = Uuid::new_v4();
        let store = Store {
            id: store_id,
            owner_user_id: owner.id,
            name,
            slug,
            status: StoreStatus::Active,
            callback_url,
            callback_secret: Some(generate_callback_secret()),
            provider_username,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        };

        let owner_member = StoreMember {
            id: Uuid::new_v4(),
            store_id,
            user_id: owner.id,
            store_role: StoreRole::Owner,
            status: StoreMemberStatus::Active,
            invited_by: Some(actor.user_id),
            created_at: now,
            updated_at: now,
        };

        let creator_member =
            if actor.platform_role == PlatformRole::Admin && actor.user_id != owner.id {
                Some(StoreMember {
                    id: Uuid::new_v4(),
                    store_id,
                    user_id: actor.user_id,
                    store_role: StoreRole::Manager,
                    status: StoreMemberStatus::Active,
                    invited_by: Some(actor.user_id),
                    created_at: now,
                    updated_at: now,
                })
            } else {
                None
            };

        let created_store = self
            .repository
            .create_store(store, owner_member, creator_member)
            .await?;
        let summary = self
            .repository
            .find_store_summary_by_id(created_store.id)
            .await?
            .ok_or_else(|| AppError::NotFound("Store not found after create".into()))?;

        self.write_audit_log(
            actor.user_id,
            "store.create",
            summary.id,
            json!({
                "owner_user_id": summary.owner_user_id,
                "slug": summary.slug,
                "provider_username": summary.provider_username,
            }),
        )
        .await?;

        Ok(summary)
    }

    pub async fn update_store(
        &self,
        store_id: Uuid,
        req: UpdateStoreRequest,
        actor: &AuthenticatedUser,
    ) -> Result<StoreSummary, AppError> {
        if !has_capability(actor, Capability::StoreUpdate, Some(store_id)) {
            return Err(AppError::Forbidden(
                "You do not have permission to update this store".into(),
            ));
        }

        let mut store = self
            .repository
            .find_store_by_id(store_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Store not found".into()))?;

        if let Some(name) = req.name {
            let name = name.trim();
            if name.is_empty() {
                return Err(AppError::BadRequest("Store name is required".into()));
            }

            store.name = name.to_string();
        }

        if let Some(slug) = req.slug {
            let slug = normalize_slug(&slug);
            if slug.is_empty() {
                return Err(AppError::BadRequest("Store slug is required".into()));
            }

            if let Some(existing_store) = self.repository.find_store_by_slug(&slug).await? {
                if existing_store.id != store_id {
                    return Err(AppError::Conflict("Store slug is already in use".into()));
                }
            }

            store.slug = slug;
        }

        if let Some(status) = req.status {
            if actor.platform_role != PlatformRole::Dev {
                return Err(AppError::Forbidden(
                    "Only dev can update store status".into(),
                ));
            }

            store.status = status;
        }

        if req.owner_user_id.is_some() || req.owner_email.is_some() {
            if actor.platform_role != PlatformRole::Dev {
                return Err(AppError::Forbidden(
                    "Only dev can reassign store owners".into(),
                ));
            }

            let owner = self
                .resolve_target_user(req.owner_user_id, req.owner_email.as_deref())
                .await?;
            validate_store_owner_role(&owner.role)?;
            store.owner_user_id = owner.id;
        }

        if let Some(provider_username) = req.provider_username {
            let provider_username = provider_username.trim();
            if provider_username.is_empty() {
                return Err(AppError::BadRequest("Provider username is required".into()));
            }

            store.provider_username = provider_username.to_string();
        }

        if req.callback_url.is_some() {
            let callback_url = normalize_optional_string(req.callback_url);
            validate_callback_url(callback_url.as_deref())?;
            store.callback_url = callback_url;
        }

        store.updated_at = Utc::now();
        self.repository.update_store(store).await?;

        let summary = self
            .repository
            .find_store_summary_by_id(store_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Store not found after update".into()))?;

        self.write_audit_log(
            actor.user_id,
            "store.update",
            summary.id,
            json!({
                "owner_user_id": summary.owner_user_id,
                "slug": summary.slug,
                "status": summary.status,
                "provider_username": summary.provider_username,
            }),
        )
        .await?;

        Ok(summary)
    }

    pub async fn list_members(
        &self,
        store_id: Uuid,
        actor: &AuthenticatedUser,
    ) -> Result<Vec<StoreMemberDetail>, AppError> {
        if !has_capability(actor, Capability::StoreMemberRead, Some(store_id)) {
            return Err(AppError::Forbidden(
                "You do not have permission to view store members".into(),
            ));
        }

        self.repository
            .list_members(store_id)
            .await
            .map_err(Into::into)
    }

    pub async fn add_member(
        &self,
        store_id: Uuid,
        req: AddStoreMemberRequest,
        actor: &AuthenticatedUser,
    ) -> Result<StoreMemberDetail, AppError> {
        if !has_capability(actor, Capability::StoreMemberManage, Some(store_id)) {
            return Err(AppError::Forbidden(
                "You do not have permission to manage store members".into(),
            ));
        }

        let store = self
            .repository
            .find_store_by_id(store_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Store not found".into()))?;
        let target_user = self
            .resolve_target_user(req.user_id, req.user_email.as_deref())
            .await?;
        validate_store_member_role(&target_user.role)?;

        if target_user.id == store.owner_user_id {
            return Err(AppError::Conflict(
                "The store owner already has membership access".into(),
            ));
        }

        let now = Utc::now();
        let detail = self
            .repository
            .upsert_member(StoreMember {
                id: Uuid::new_v4(),
                store_id,
                user_id: target_user.id,
                store_role: req.store_role,
                status: StoreMemberStatus::Active,
                invited_by: Some(actor.user_id),
                created_at: now,
                updated_at: now,
            })
            .await?;

        Ok(detail)
    }

    pub async fn update_member(
        &self,
        store_id: Uuid,
        member_id: Uuid,
        req: UpdateStoreMemberRequest,
        actor: &AuthenticatedUser,
    ) -> Result<StoreMemberDetail, AppError> {
        if !has_capability(actor, Capability::StoreMemberManage, Some(store_id)) {
            return Err(AppError::Forbidden(
                "You do not have permission to manage store members".into(),
            ));
        }

        let mut member = self
            .repository
            .find_member_by_id(store_id, member_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Store member not found".into()))?;

        if member.store_role == StoreRole::Owner {
            return Err(AppError::BadRequest(
                "Owner membership cannot be modified in v1".into(),
            ));
        }

        if let Some(store_role) = req.store_role {
            member.store_role = store_role;
        }

        if let Some(status) = req.status {
            member.status = status;
        }

        member.updated_at = Utc::now();
        self.repository
            .update_member(member)
            .await
            .map_err(Into::into)
    }

    pub async fn remove_member(
        &self,
        store_id: Uuid,
        member_id: Uuid,
        actor: &AuthenticatedUser,
    ) -> Result<(), AppError> {
        if !has_capability(actor, Capability::StoreMemberManage, Some(store_id)) {
            return Err(AppError::Forbidden(
                "You do not have permission to manage store members".into(),
            ));
        }

        let member = self
            .repository
            .find_member_by_id(store_id, member_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Store member not found".into()))?;

        if member.store_role == StoreRole::Owner {
            return Err(AppError::BadRequest(
                "Owner membership cannot be removed in v1".into(),
            ));
        }

        self.repository
            .deactivate_member(store_id, member_id)
            .await?;
        Ok(())
    }

    async fn resolve_target_user(
        &self,
        user_id: Option<Uuid>,
        user_email: Option<&str>,
    ) -> Result<crate::modules::users::domain::entity::User, AppError> {
        let user = match (
            user_id,
            user_email.map(str::trim).filter(|value| !value.is_empty()),
        ) {
            (Some(user_id), _) => self.user_repository.find_by_id(user_id).await?,
            (None, Some(email)) => self.user_repository.find_by_email(email).await?,
            (None, None) => {
                return Err(AppError::BadRequest(
                    "Either user_id or email must be provided".into(),
                ))
            }
        };

        let user = user.ok_or_else(|| AppError::NotFound("User not found".into()))?;

        if user.status != crate::modules::users::domain::entity::UserStatus::Active {
            return Err(AppError::BadRequest("Target user must be active".into()));
        }

        Ok(user)
    }

    async fn write_audit_log(
        &self,
        actor_user_id: Uuid,
        action: &str,
        target_id: Uuid,
        payload_json: serde_json::Value,
    ) -> Result<(), AppError> {
        self.audit_repository
            .insert(AuditLogEntry {
                actor_user_id: Some(actor_user_id),
                action: action.to_string(),
                target_type: Some("store".to_string()),
                target_id: Some(target_id),
                payload_json,
            })
            .await?;

        Ok(())
    }
}

fn store_scope(actor: &AuthenticatedUser) -> Option<Uuid> {
    if actor.platform_role == PlatformRole::Dev || actor.platform_role == PlatformRole::Superadmin {
        None
    } else {
        Some(actor.user_id)
    }
}

fn normalize_slug(value: &str) -> String {
    let mut slug = String::new();
    let mut last_was_dash = false;

    for character in value.trim().chars() {
        let lower = character.to_ascii_lowercase();
        if lower.is_ascii_alphanumeric() {
            slug.push(lower);
            last_was_dash = false;
        } else if !last_was_dash {
            slug.push('-');
            last_was_dash = true;
        }
    }

    slug.trim_matches('-').to_string()
}

fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|inner| inner.trim().to_string())
        .filter(|inner| !inner.is_empty())
}

fn validate_callback_url(value: Option<&str>) -> Result<(), AppError> {
    if let Some(callback_url) = value {
        if !callback_url.starts_with("http://") && !callback_url.starts_with("https://") {
            return Err(AppError::BadRequest(
                "Callback URL must start with http:// or https://".into(),
            ));
        }
    }

    Ok(())
}

fn validate_store_owner_role(role: &PlatformRole) -> Result<(), AppError> {
    if matches!(role, PlatformRole::Dev | PlatformRole::Superadmin) {
        return Err(AppError::BadRequest(
            "Dev and superadmin users cannot be store owners".into(),
        ));
    }

    Ok(())
}

fn validate_store_member_role(role: &PlatformRole) -> Result<(), AppError> {
    if matches!(role, PlatformRole::Dev | PlatformRole::Superadmin) {
        return Err(AppError::BadRequest(
            "Dev and superadmin users cannot be added as store members".into(),
        ));
    }

    Ok(())
}

fn generate_callback_secret() -> String {
    let mut random_bytes = [0_u8; 16];
    rand::thread_rng().fill_bytes(&mut random_bytes);

    let random_hex = random_bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();

    format!("cb_{}_{}", Uuid::new_v4().simple(), random_hex)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;

    use super::*;
    use crate::modules::users::domain::entity::{User, UserStatus};
    use crate::shared::audit::AuditLogRepository;

    #[derive(Default)]
    struct MockStoreRepository {
        stores: Mutex<HashMap<Uuid, Store>>,
        members: Mutex<HashMap<Uuid, Vec<StoreMember>>>,
    }

    #[async_trait]
    impl StoreRepository for MockStoreRepository {
        async fn list_stores(
            &self,
            _limit: i64,
            _offset: i64,
            _search: Option<&str>,
            _status: Option<StoreStatus>,
            _user_scope: Option<Uuid>,
        ) -> anyhow::Result<Vec<StoreSummary>> {
            Ok(vec![])
        }

        async fn count_stores(
            &self,
            _search: Option<&str>,
            _status: Option<StoreStatus>,
            _user_scope: Option<Uuid>,
        ) -> anyhow::Result<i64> {
            Ok(0)
        }

        async fn find_store_by_id(&self, id: Uuid) -> anyhow::Result<Option<Store>> {
            Ok(self.stores.lock().unwrap().get(&id).cloned())
        }

        async fn find_store_summary_by_id(&self, id: Uuid) -> anyhow::Result<Option<StoreSummary>> {
            Ok(self
                .stores
                .lock()
                .unwrap()
                .get(&id)
                .map(|store| StoreSummary {
                    id: store.id,
                    owner_user_id: store.owner_user_id,
                    owner_name: "Owner".into(),
                    owner_email: "owner@example.com".into(),
                    name: store.name.clone(),
                    slug: store.slug.clone(),
                    status: store.status.clone(),
                    callback_url: store.callback_url.clone(),
                    provider_username: store.provider_username.clone(),
                    created_at: store.created_at,
                    updated_at: store.updated_at,
                }))
        }

        async fn find_store_by_slug(&self, slug: &str) -> anyhow::Result<Option<Store>> {
            Ok(self
                .stores
                .lock()
                .unwrap()
                .values()
                .find(|store| store.slug == slug)
                .cloned())
        }

        async fn create_store(
            &self,
            store: Store,
            owner_member: StoreMember,
            creator_member: Option<StoreMember>,
        ) -> anyhow::Result<Store> {
            self.stores.lock().unwrap().insert(store.id, store.clone());
            let mut members = vec![owner_member];
            if let Some(member) = creator_member {
                members.push(member);
            }
            self.members.lock().unwrap().insert(store.id, members);
            Ok(store)
        }

        async fn update_store(&self, store: Store) -> anyhow::Result<Store> {
            self.stores.lock().unwrap().insert(store.id, store.clone());
            Ok(store)
        }

        async fn list_members(&self, store_id: Uuid) -> anyhow::Result<Vec<StoreMemberDetail>> {
            Ok(self
                .members
                .lock()
                .unwrap()
                .get(&store_id)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(|member| StoreMemberDetail {
                    id: member.id,
                    store_id: member.store_id,
                    user_id: member.user_id,
                    user_name: "Member".into(),
                    user_email: format!("{}@example.com", member.user_id),
                    user_platform_role: "user".into(),
                    store_role: member.store_role,
                    status: member.status,
                    invited_by: member.invited_by,
                    created_at: member.created_at,
                    updated_at: member.updated_at,
                })
                .collect())
        }

        async fn find_member_by_id(
            &self,
            store_id: Uuid,
            member_id: Uuid,
        ) -> anyhow::Result<Option<StoreMember>> {
            Ok(self
                .members
                .lock()
                .unwrap()
                .get(&store_id)
                .and_then(|members| {
                    members
                        .iter()
                        .find(|member| member.id == member_id)
                        .cloned()
                }))
        }

        async fn find_member_by_user_id(
            &self,
            store_id: Uuid,
            user_id: Uuid,
        ) -> anyhow::Result<Option<StoreMember>> {
            Ok(self
                .members
                .lock()
                .unwrap()
                .get(&store_id)
                .and_then(|members| {
                    members
                        .iter()
                        .find(|member| member.user_id == user_id)
                        .cloned()
                }))
        }

        async fn upsert_member(&self, member: StoreMember) -> anyhow::Result<StoreMemberDetail> {
            let mut members_lock = self.members.lock().unwrap();
            let members = members_lock.entry(member.store_id).or_default();
            if let Some(existing) = members
                .iter_mut()
                .find(|existing| existing.user_id == member.user_id)
            {
                *existing = member.clone();
            } else {
                members.push(member.clone());
            }

            Ok(StoreMemberDetail {
                id: member.id,
                store_id: member.store_id,
                user_id: member.user_id,
                user_name: "Member".into(),
                user_email: format!("{}@example.com", member.user_id),
                user_platform_role: "user".into(),
                store_role: member.store_role,
                status: member.status,
                invited_by: member.invited_by,
                created_at: member.created_at,
                updated_at: member.updated_at,
            })
        }

        async fn update_member(&self, member: StoreMember) -> anyhow::Result<StoreMemberDetail> {
            let mut members_lock = self.members.lock().unwrap();
            let members = members_lock.entry(member.store_id).or_default();
            if let Some(existing) = members.iter_mut().find(|existing| existing.id == member.id) {
                *existing = member.clone();
            }

            Ok(StoreMemberDetail {
                id: member.id,
                store_id: member.store_id,
                user_id: member.user_id,
                user_name: "Member".into(),
                user_email: format!("{}@example.com", member.user_id),
                user_platform_role: "user".into(),
                store_role: member.store_role,
                status: member.status,
                invited_by: member.invited_by,
                created_at: member.created_at,
                updated_at: member.updated_at,
            })
        }

        async fn deactivate_member(&self, store_id: Uuid, member_id: Uuid) -> anyhow::Result<()> {
            if let Some(member) = self
                .members
                .lock()
                .unwrap()
                .get_mut(&store_id)
                .and_then(|members| members.iter_mut().find(|member| member.id == member_id))
            {
                member.status = StoreMemberStatus::Inactive;
            }

            Ok(())
        }
    }

    #[derive(Default)]
    struct MockUserRepository {
        users: Mutex<HashMap<Uuid, User>>,
    }

    #[async_trait]
    impl UserRepository for MockUserRepository {
        async fn find_by_id(&self, id: Uuid) -> anyhow::Result<Option<User>> {
            Ok(self.users.lock().unwrap().get(&id).cloned())
        }

        async fn find_by_email(&self, email: &str) -> anyhow::Result<Option<User>> {
            Ok(self
                .users
                .lock()
                .unwrap()
                .values()
                .find(|user| user.email == email)
                .cloned())
        }

        async fn list_memberships(
            &self,
            _user_id: Uuid,
        ) -> anyhow::Result<HashMap<Uuid, StoreRole>> {
            Ok(HashMap::new())
        }

        async fn user_exists_in_scope(
            &self,
            _actor_user_id: Uuid,
            _target_user_id: Uuid,
        ) -> anyhow::Result<bool> {
            Ok(false)
        }

        async fn list_users(
            &self,
            _limit: i64,
            _offset: i64,
            _role_filter: Option<PlatformRole>,
            _search: Option<&str>,
            _store_scope: Option<Uuid>,
        ) -> anyhow::Result<Vec<User>> {
            Ok(vec![])
        }

        async fn count_users(
            &self,
            _role_filter: Option<PlatformRole>,
            _search: Option<&str>,
            _store_scope: Option<Uuid>,
        ) -> anyhow::Result<i64> {
            Ok(0)
        }

        async fn create(&self, user: User) -> anyhow::Result<User> {
            self.users.lock().unwrap().insert(user.id, user.clone());
            Ok(user)
        }

        async fn update(&self, user: User) -> anyhow::Result<User> {
            self.users.lock().unwrap().insert(user.id, user.clone());
            Ok(user)
        }

        async fn update_status(&self, _id: Uuid, _status: UserStatus) -> anyhow::Result<()> {
            Ok(())
        }
    }

    #[derive(Default)]
    struct MockAuditRepository;

    #[async_trait]
    impl AuditLogRepository for MockAuditRepository {
        async fn insert(&self, _entry: AuditLogEntry) -> anyhow::Result<()> {
            Ok(())
        }
    }

    fn sample_user(id: Uuid, role: PlatformRole) -> User {
        User {
            id,
            name: "User".into(),
            email: format!("{id}@example.com"),
            password_hash: "hash".into(),
            role,
            status: UserStatus::Active,
            created_by: None,
            last_login_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            deleted_at: None,
        }
    }

    #[tokio::test]
    async fn admin_created_store_gets_owner_and_manager_membership() {
        let store_repository = Arc::new(MockStoreRepository::default());
        let user_repository = Arc::new(MockUserRepository::default());
        let audit_repository = Arc::new(MockAuditRepository);
        let service = StoreService::new(
            store_repository.clone(),
            user_repository.clone(),
            audit_repository,
        );

        let admin_id = Uuid::new_v4();
        let owner_id = Uuid::new_v4();
        user_repository
            .users
            .lock()
            .unwrap()
            .insert(owner_id, sample_user(owner_id, PlatformRole::User));

        let actor = AuthenticatedUser {
            user_id: admin_id,
            platform_role: PlatformRole::Admin,
            memberships: HashMap::new(),
        };

        let store = service
            .create_store(
                CreateStoreRequest {
                    owner_user_id: Some(owner_id),
                    owner_email: None,
                    name: "Example Store".into(),
                    slug: "example-store".into(),
                    callback_url: Some("https://example.com/callback".into()),
                    provider_username: "provider-user".into(),
                },
                &actor,
            )
            .await
            .unwrap();

        let members = store_repository.members.lock().unwrap();
        let store_members = members.get(&store.id).unwrap();
        assert_eq!(store_members.len(), 2);
        assert!(store_members
            .iter()
            .any(|member| member.user_id == owner_id && member.store_role == StoreRole::Owner));
        assert!(store_members
            .iter()
            .any(|member| member.user_id == admin_id && member.store_role == StoreRole::Manager));
    }

    #[tokio::test]
    async fn owner_can_update_store_metadata() {
        let store_repository = Arc::new(MockStoreRepository::default());
        let user_repository = Arc::new(MockUserRepository::default());
        let audit_repository = Arc::new(MockAuditRepository);
        let service = StoreService::new(store_repository.clone(), user_repository, audit_repository);

        let store_id = Uuid::new_v4();
        let owner_id = Uuid::new_v4();
        store_repository.stores.lock().unwrap().insert(
            store_id,
            Store {
                id: store_id,
                owner_user_id: owner_id,
                name: "Before".into(),
                slug: "before-store".into(),
                status: StoreStatus::Active,
                callback_url: Some("https://before.example.com/callback".into()),
                callback_secret: Some("secret".into()),
                provider_username: "before-provider".into(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                deleted_at: None,
            },
        );

        let actor = AuthenticatedUser {
            user_id: owner_id,
            platform_role: PlatformRole::User,
            memberships: HashMap::from([(store_id, StoreRole::Owner)]),
        };

        let updated = service
            .update_store(
                store_id,
                UpdateStoreRequest {
                    owner_user_id: None,
                    owner_email: None,
                    name: Some("After".into()),
                    slug: Some("after-store".into()),
                    status: None,
                    callback_url: Some("https://after.example.com/callback".into()),
                    provider_username: Some("after-provider".into()),
                },
                &actor,
            )
            .await
            .unwrap();

        assert_eq!(updated.name, "After");
        assert_eq!(updated.slug, "after-store");
        assert_eq!(
            updated.callback_url.as_deref(),
            Some("https://after.example.com/callback")
        );
        assert_eq!(updated.provider_username, "after-provider");
    }

    #[tokio::test]
    async fn admin_without_membership_cannot_manage_members_for_other_store() {
        let store_repository = Arc::new(MockStoreRepository::default());
        let user_repository = Arc::new(MockUserRepository::default());
        let audit_repository = Arc::new(MockAuditRepository);
        let service = StoreService::new(
            store_repository.clone(),
            user_repository.clone(),
            audit_repository,
        );

        let store_id = Uuid::new_v4();
        let owner_id = Uuid::new_v4();
        let member_id = Uuid::new_v4();

        store_repository.stores.lock().unwrap().insert(
            store_id,
            Store {
                id: store_id,
                owner_user_id: owner_id,
                name: "Scoped Store".into(),
                slug: "scoped-store".into(),
                status: StoreStatus::Active,
                callback_url: None,
                callback_secret: Some("secret".into()),
                provider_username: "provider-user".into(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                deleted_at: None,
            },
        );
        user_repository
            .users
            .lock()
            .unwrap()
            .insert(member_id, sample_user(member_id, PlatformRole::User));

        let actor = AuthenticatedUser {
            user_id: Uuid::new_v4(),
            platform_role: PlatformRole::Admin,
            memberships: HashMap::new(),
        };

        let error = service
            .add_member(
                store_id,
                AddStoreMemberRequest {
                    user_id: Some(member_id),
                    user_email: None,
                    store_role: StoreRole::Viewer,
                },
                &actor,
            )
            .await
            .unwrap_err();

        assert!(matches!(error, AppError::Forbidden(_)));
    }

    #[tokio::test]
    async fn owner_membership_cannot_be_removed() {
        let store_repository = Arc::new(MockStoreRepository::default());
        let user_repository = Arc::new(MockUserRepository::default());
        let audit_repository = Arc::new(MockAuditRepository);
        let service =
            StoreService::new(store_repository.clone(), user_repository, audit_repository);

        let store_id = Uuid::new_v4();
        let owner_id = Uuid::new_v4();
        let owner_member_id = Uuid::new_v4();

        store_repository.stores.lock().unwrap().insert(
            store_id,
            Store {
                id: store_id,
                owner_user_id: owner_id,
                name: "Example Store".into(),
                slug: "example-store".into(),
                status: StoreStatus::Active,
                callback_url: None,
                callback_secret: Some("secret".into()),
                provider_username: "provider-user".into(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                deleted_at: None,
            },
        );
        store_repository.members.lock().unwrap().insert(
            store_id,
            vec![StoreMember {
                id: owner_member_id,
                store_id,
                user_id: owner_id,
                store_role: StoreRole::Owner,
                status: StoreMemberStatus::Active,
                invited_by: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            }],
        );

        let actor = AuthenticatedUser {
            user_id: owner_id,
            platform_role: PlatformRole::User,
            memberships: HashMap::from([(store_id, StoreRole::Owner)]),
        };

        let error = service
            .remove_member(store_id, owner_member_id, &actor)
            .await
            .unwrap_err();

        assert!(matches!(error, AppError::BadRequest(_)));
    }
}
