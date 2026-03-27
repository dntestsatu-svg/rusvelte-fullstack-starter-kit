use std::collections::HashMap;
use std::sync::Arc;

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::modules::users::domain::entity::{User, UserStatus};
use crate::modules::users::domain::repository::UserRepository;
use crate::shared::audit::{AuditLogEntry, AuditLogRepository};
use crate::shared::auth::{AuthenticatedUser, PlatformRole, StoreRole};
use crate::shared::error::AppError;

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub name: String,
    pub email: String,
    pub password: String,
    pub role: PlatformRole,
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserRequest {
    pub name: Option<String>,
    pub email: Option<String>,
    pub role: Option<PlatformRole>,
    pub status: Option<UserStatus>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct UserListFilters {
    pub role: Option<PlatformRole>,
    pub search: Option<String>,
}

pub struct UserService {
    repository: Arc<dyn UserRepository>,
    audit_repository: Arc<dyn AuditLogRepository>,
}

impl UserService {
    pub fn new(
        repository: Arc<dyn UserRepository>,
        audit_repository: Arc<dyn AuditLogRepository>,
    ) -> Self {
        Self {
            repository,
            audit_repository,
        }
    }

    pub async fn build_actor(
        &self,
        user_id: Uuid,
        platform_role: PlatformRole,
    ) -> Result<AuthenticatedUser, AppError> {
        let memberships = self.repository.list_memberships(user_id).await?;

        Ok(AuthenticatedUser {
            user_id,
            platform_role,
            memberships,
        })
    }

    pub async fn memberships_for_user(
        &self,
        user_id: Uuid,
    ) -> Result<HashMap<Uuid, StoreRole>, AppError> {
        self.repository
            .list_memberships(user_id)
            .await
            .map_err(Into::into)
    }

    pub async fn create_user(
        &self,
        req: CreateUserRequest,
        actor: &AuthenticatedUser,
    ) -> Result<User, AppError> {
        let name = req.name.trim().to_string();
        let email = req.email.trim().to_lowercase();

        if name.is_empty() {
            return Err(AppError::BadRequest("Name is required".into()));
        }

        if email.is_empty() {
            return Err(AppError::BadRequest("Email is required".into()));
        }

        if req.password.len() < 8 {
            return Err(AppError::BadRequest(
                "Password must be at least 8 characters".into(),
            ));
        }

        if req.role == PlatformRole::Dev {
            return Err(AppError::Forbidden(
                "Creating additional dev users is not allowed".into(),
            ));
        }

        if actor.platform_role == PlatformRole::Admin && req.role != PlatformRole::User {
            return Err(AppError::Forbidden(
                "Admin can only create users with the user role".into(),
            ));
        }

        if self.repository.find_by_email(&email).await?.is_some() {
            return Err(AppError::Conflict(
                "User with this email already exists".into(),
            ));
        }

        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let password_hash = argon2
            .hash_password(req.password.as_bytes(), &salt)
            .map_err(|error| AppError::Internal(anyhow::anyhow!(error.to_string())))?
            .to_string();

        let user = User {
            id: Uuid::new_v4(),
            name,
            email,
            password_hash,
            role: req.role,
            status: UserStatus::Active,
            created_by: Some(actor.user_id),
            last_login_at: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            deleted_at: None,
        };

        let created_user = self.repository.create(user).await?;
        self.write_audit_log(
            actor.user_id,
            "user.create",
            Some(created_user.id),
            json!({
                "email": created_user.email,
                "role": created_user.role,
                "status": created_user.status,
            }),
        )
        .await?;

        Ok(created_user)
    }

    pub async fn list_users(
        &self,
        limit: i64,
        offset: i64,
        filters: UserListFilters,
        actor: &AuthenticatedUser,
    ) -> Result<Vec<User>, AppError> {
        self.repository
            .list_users(
                limit,
                offset,
                filters.role,
                filters.search.as_deref(),
                scope_user_id(actor),
            )
            .await
            .map_err(Into::into)
    }

    pub async fn count_users(
        &self,
        filters: UserListFilters,
        actor: &AuthenticatedUser,
    ) -> Result<i64, AppError> {
        self.repository
            .count_users(
                filters.role,
                filters.search.as_deref(),
                scope_user_id(actor),
            )
            .await
            .map_err(Into::into)
    }

    pub async fn get_user(
        &self,
        id: Uuid,
        actor: &AuthenticatedUser,
    ) -> Result<Option<User>, AppError> {
        if actor.platform_role == PlatformRole::Admin
            && actor.user_id != id
            && !self
                .repository
                .user_exists_in_scope(actor.user_id, id)
                .await?
        {
            return Ok(None);
        }

        self.repository.find_by_id(id).await.map_err(Into::into)
    }

    pub async fn update_user(
        &self,
        id: Uuid,
        req: UpdateUserRequest,
        actor: &AuthenticatedUser,
    ) -> Result<User, AppError> {
        let mut user = self
            .repository
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound("User not found".into()))?;

        if actor.user_id == id && (req.role.is_some() || req.status.is_some()) {
            return Err(AppError::BadRequest(
                "You cannot change your own role or account status".into(),
            ));
        }

        if let Some(name) = req.name {
            let name = name.trim();
            if name.is_empty() {
                return Err(AppError::BadRequest("Name is required".into()));
            }

            user.name = name.to_string();
        }

        if let Some(email) = req.email {
            let email = email.trim().to_lowercase();
            if email.is_empty() {
                return Err(AppError::BadRequest("Email is required".into()));
            }

            if let Some(existing_user) = self.repository.find_by_email(&email).await? {
                if existing_user.id != id {
                    return Err(AppError::Conflict(
                        "User with this email already exists".into(),
                    ));
                }
            }

            user.email = email;
        }

        if let Some(role) = req.role {
            if role == PlatformRole::Dev {
                return Err(AppError::Forbidden(
                    "Assigning the dev role is not allowed".into(),
                ));
            }

            user.role = role;
        }

        if let Some(status) = req.status {
            user.status = status;
        }

        user.updated_at = chrono::Utc::now();
        let updated_user = self.repository.update(user).await?;

        self.write_audit_log(
            actor.user_id,
            "user.update",
            Some(updated_user.id),
            json!({
                "email": updated_user.email,
                "role": updated_user.role,
                "status": updated_user.status,
            }),
        )
        .await?;

        Ok(updated_user)
    }

    pub async fn disable_user(&self, id: Uuid, actor: &AuthenticatedUser) -> Result<(), AppError> {
        if actor.user_id == id {
            return Err(AppError::BadRequest(
                "You cannot disable your own account".into(),
            ));
        }

        let user = self
            .repository
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound("User not found".into()))?;

        if user.status == UserStatus::Suspended {
            return Ok(());
        }

        self.repository
            .update_status(id, UserStatus::Suspended)
            .await?;
        self.write_audit_log(
            actor.user_id,
            "user.disable",
            Some(id),
            json!({
                "previous_status": user.status,
                "new_status": UserStatus::Suspended,
            }),
        )
        .await?;

        Ok(())
    }

    async fn write_audit_log(
        &self,
        actor_user_id: Uuid,
        action: &str,
        target_id: Option<Uuid>,
        payload_json: serde_json::Value,
    ) -> Result<(), AppError> {
        self.audit_repository
            .insert(AuditLogEntry {
                actor_user_id: Some(actor_user_id),
                action: action.to_string(),
                target_type: Some("user".to_string()),
                target_id,
                payload_json,
            })
            .await?;

        Ok(())
    }
}

fn scope_user_id(actor: &AuthenticatedUser) -> Option<Uuid> {
    if actor.platform_role == PlatformRole::Admin {
        Some(actor.user_id)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::shared::audit::AuditLogRepository;
    use async_trait::async_trait;

    struct MockUserRepository {
        users: Mutex<HashMap<Uuid, User>>,
        memberships: Mutex<HashMap<Uuid, HashMap<Uuid, StoreRole>>>,
    }

    impl MockUserRepository {
        fn new() -> Self {
            Self {
                users: Mutex::new(HashMap::new()),
                memberships: Mutex::new(HashMap::new()),
            }
        }
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
            user_id: Uuid,
        ) -> anyhow::Result<HashMap<Uuid, StoreRole>> {
            Ok(self
                .memberships
                .lock()
                .unwrap()
                .get(&user_id)
                .cloned()
                .unwrap_or_default())
        }

        async fn user_exists_in_scope(
            &self,
            actor_user_id: Uuid,
            target_user_id: Uuid,
        ) -> anyhow::Result<bool> {
            if actor_user_id == target_user_id {
                return Ok(true);
            }

            let memberships = self.memberships.lock().unwrap();
            let actor_store_ids: Vec<Uuid> = memberships
                .get(&actor_user_id)
                .cloned()
                .unwrap_or_default()
                .into_keys()
                .collect();

            let target_store_ids: Vec<Uuid> = memberships
                .get(&target_user_id)
                .cloned()
                .unwrap_or_default()
                .into_keys()
                .collect();

            Ok(actor_store_ids
                .iter()
                .any(|store_id| target_store_ids.contains(store_id)))
        }

        async fn list_users(
            &self,
            _limit: i64,
            _offset: i64,
            _role_filter: Option<PlatformRole>,
            _search: Option<&str>,
            _store_scope: Option<Uuid>,
        ) -> anyhow::Result<Vec<User>> {
            Ok(self.users.lock().unwrap().values().cloned().collect())
        }

        async fn count_users(
            &self,
            _role_filter: Option<PlatformRole>,
            _search: Option<&str>,
            _store_scope: Option<Uuid>,
        ) -> anyhow::Result<i64> {
            Ok(self.users.lock().unwrap().len() as i64)
        }

        async fn create(&self, user: User) -> anyhow::Result<User> {
            self.users.lock().unwrap().insert(user.id, user.clone());
            Ok(user)
        }

        async fn update(&self, user: User) -> anyhow::Result<User> {
            self.users.lock().unwrap().insert(user.id, user.clone());
            Ok(user)
        }

        async fn update_status(&self, id: Uuid, status: UserStatus) -> anyhow::Result<()> {
            if let Some(user) = self.users.lock().unwrap().get_mut(&id) {
                user.status = status;
            }

            Ok(())
        }
    }

    #[derive(Default)]
    struct MockAuditRepository {
        entries: Mutex<Vec<AuditLogEntry>>,
    }

    #[async_trait]
    impl AuditLogRepository for MockAuditRepository {
        async fn insert(&self, entry: AuditLogEntry) -> anyhow::Result<()> {
            self.entries.lock().unwrap().push(entry);
            Ok(())
        }
    }

    fn sample_user(id: Uuid, role: PlatformRole) -> User {
        User {
            id,
            name: "Sample User".into(),
            email: format!("{id}@example.com"),
            password_hash: "hashed".into(),
            role,
            status: UserStatus::Active,
            created_by: None,
            last_login_at: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            deleted_at: None,
        }
    }

    fn dev_actor() -> AuthenticatedUser {
        AuthenticatedUser {
            user_id: Uuid::new_v4(),
            platform_role: PlatformRole::Dev,
            memberships: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn create_user_writes_audit_log() {
        let repository = Arc::new(MockUserRepository::new());
        let audit_repository = Arc::new(MockAuditRepository::default());
        let service = UserService::new(repository.clone(), audit_repository.clone());
        let actor = dev_actor();

        let created_user = service
            .create_user(
                CreateUserRequest {
                    name: "Alice".into(),
                    email: "alice@example.com".into(),
                    password: "password123".into(),
                    role: PlatformRole::Admin,
                },
                &actor,
            )
            .await
            .unwrap();

        assert_eq!(created_user.email, "alice@example.com");
        assert_eq!(created_user.role, PlatformRole::Admin);

        let entries = audit_repository.entries.lock().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].action, "user.create");
        assert_eq!(entries[0].target_id, Some(created_user.id));
    }

    #[tokio::test]
    async fn admin_cannot_create_privileged_user() {
        let repository = Arc::new(MockUserRepository::new());
        let audit_repository = Arc::new(MockAuditRepository::default());
        let service = UserService::new(repository, audit_repository);
        let actor = AuthenticatedUser {
            user_id: Uuid::new_v4(),
            platform_role: PlatformRole::Admin,
            memberships: HashMap::new(),
        };

        let error = service
            .create_user(
                CreateUserRequest {
                    name: "Alice".into(),
                    email: "alice@example.com".into(),
                    password: "password123".into(),
                    role: PlatformRole::Admin,
                },
                &actor,
            )
            .await
            .unwrap_err();

        assert!(matches!(error, AppError::Forbidden(_)));
    }

    #[tokio::test]
    async fn admin_can_only_read_users_in_scope() {
        let repository = Arc::new(MockUserRepository::new());
        let audit_repository = Arc::new(MockAuditRepository::default());
        let service = UserService::new(repository.clone(), audit_repository);

        let admin_id = Uuid::new_v4();
        let shared_store_id = Uuid::new_v4();
        let scoped_user = sample_user(Uuid::new_v4(), PlatformRole::User);
        let outsider_user = sample_user(Uuid::new_v4(), PlatformRole::User);

        repository
            .users
            .lock()
            .unwrap()
            .insert(scoped_user.id, scoped_user.clone());
        repository
            .users
            .lock()
            .unwrap()
            .insert(outsider_user.id, outsider_user.clone());

        repository.memberships.lock().unwrap().insert(
            admin_id,
            HashMap::from([(shared_store_id, StoreRole::Manager)]),
        );
        repository.memberships.lock().unwrap().insert(
            scoped_user.id,
            HashMap::from([(shared_store_id, StoreRole::Staff)]),
        );

        let actor = AuthenticatedUser {
            user_id: admin_id,
            platform_role: PlatformRole::Admin,
            memberships: HashMap::new(),
        };

        assert!(service
            .get_user(scoped_user.id, &actor)
            .await
            .unwrap()
            .is_some());
        assert!(service
            .get_user(outsider_user.id, &actor)
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn disable_user_rejects_self_disable() {
        let repository = Arc::new(MockUserRepository::new());
        let audit_repository = Arc::new(MockAuditRepository::default());
        let service = UserService::new(repository.clone(), audit_repository);
        let actor = dev_actor();

        repository
            .users
            .lock()
            .unwrap()
            .insert(actor.user_id, sample_user(actor.user_id, PlatformRole::Dev));

        let error = service
            .disable_user(actor.user_id, &actor)
            .await
            .unwrap_err();
        assert!(matches!(error, AppError::BadRequest(_)));
    }
}
