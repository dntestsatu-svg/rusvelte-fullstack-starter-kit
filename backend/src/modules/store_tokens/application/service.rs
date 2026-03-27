use std::sync::Arc;

use chrono::Utc;
use rand::{distributions::Alphanumeric, Rng, RngCore};
use serde::Deserialize;
use serde_json::json;
use sqlx::Error as SqlxError;
use uuid::Uuid;

use crate::infrastructure::security::argon2::{hash_secret, verify_secret};
use crate::modules::store_tokens::domain::entity::{
    is_store_api_token_expired, masked_display_prefix, CreateStoreApiTokenResult,
    NewStoreApiTokenRecord, RevokeStoreApiTokenResult, StoreApiTokenAuthContext,
    StoreApiTokenMetadata, StoreApiTokenRecord, STORE_API_TOKEN_LOOKUP_LENGTH,
    STORE_API_TOKEN_PREFIX,
};
use crate::modules::store_tokens::domain::repository::StoreTokenRepository;
use crate::shared::audit::{AuditLogEntry, AuditLogRepository};
use crate::shared::auth::{has_capability, AuthenticatedUser, Capability};
use crate::shared::error::AppError;

#[derive(Debug, Deserialize)]
pub struct CreateStoreApiTokenRequest {
    pub name: String,
}

pub struct StoreTokenService {
    repository: Arc<dyn StoreTokenRepository>,
    audit_repository: Arc<dyn AuditLogRepository>,
}

impl StoreTokenService {
    pub fn new(
        repository: Arc<dyn StoreTokenRepository>,
        audit_repository: Arc<dyn AuditLogRepository>,
    ) -> Self {
        Self {
            repository,
            audit_repository,
        }
    }

    pub async fn list_tokens(
        &self,
        store_id: Uuid,
        actor: &AuthenticatedUser,
    ) -> Result<Vec<StoreApiTokenMetadata>, AppError> {
        self.ensure_store_exists(store_id).await?;
        self.ensure_capability(actor, Capability::StoreTokenRead, store_id)?;

        let tokens = self.repository.list_active_tokens(store_id).await?;
        Ok(tokens.into_iter().map(map_metadata).collect())
    }

    pub async fn create_token(
        &self,
        store_id: Uuid,
        request: CreateStoreApiTokenRequest,
        actor: &AuthenticatedUser,
    ) -> Result<CreateStoreApiTokenResult, AppError> {
        self.ensure_store_exists(store_id).await?;
        self.ensure_capability(actor, Capability::StoreTokenManage, store_id)?;

        let name = request.name.trim().to_string();
        if name.is_empty() {
            return Err(AppError::BadRequest("Token name is required".into()));
        }

        for _ in 0..3 {
            let (token_prefix, plaintext_token) = generate_store_api_token();
            let token_hash = hash_secret(&plaintext_token)
                .map_err(|error| AppError::Internal(anyhow::anyhow!(error.to_string())))?;
            let now = Utc::now();

            let record = self
                .repository
                .insert_token(NewStoreApiTokenRecord {
                    id: Uuid::new_v4(),
                    store_id,
                    name: name.clone(),
                    token_prefix: token_prefix.clone(),
                    token_hash,
                    expires_at: None,
                    created_by: actor.user_id,
                    created_at: now,
                })
                .await;

            match record {
                Ok(record) => {
                    let token = map_metadata(record.clone());
                    self.write_audit_log(
                        actor.user_id,
                        "store.token.create",
                        record.id,
                        json!({
                            "store_id": store_id,
                            "name": record.name,
                            "display_prefix": token.display_prefix,
                        }),
                    )
                    .await?;

                    return Ok(CreateStoreApiTokenResult {
                        plaintext_token,
                        token,
                    });
                }
                Err(error) if is_unique_prefix_conflict(&error) => continue,
                Err(error) => return Err(error.into()),
            }
        }

        Err(AppError::Conflict(
            "Unable to allocate a unique store token prefix".into(),
        ))
    }

    pub async fn revoke_token(
        &self,
        store_id: Uuid,
        token_id: Uuid,
        actor: &AuthenticatedUser,
    ) -> Result<RevokeStoreApiTokenResult, AppError> {
        self.ensure_store_exists(store_id).await?;
        self.ensure_capability(actor, Capability::StoreTokenManage, store_id)?;

        let token = self
            .repository
            .find_token_by_id(store_id, token_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Store token not found".into()))?;

        if token.revoked_at.is_some() {
            return Ok(RevokeStoreApiTokenResult::AlreadyRevoked);
        }

        let revoked_at = Utc::now();
        let was_revoked = self
            .repository
            .mark_token_revoked(store_id, token_id, revoked_at)
            .await?;

        if !was_revoked {
            return Ok(RevokeStoreApiTokenResult::AlreadyRevoked);
        }

        self.write_audit_log(
            actor.user_id,
            "store.token.revoke",
            token.id,
            json!({
                "store_id": store_id,
                "name": token.name,
                "display_prefix": masked_display_prefix(&token.token_prefix),
            }),
        )
        .await?;

        Ok(RevokeStoreApiTokenResult::Revoked)
    }

    pub async fn resolve_bearer_token(
        &self,
        plaintext_token: &str,
    ) -> Result<StoreApiTokenAuthContext, AppError> {
        let token_prefix = extract_lookup_prefix(plaintext_token)
            .ok_or_else(|| AppError::Unauthorized("Invalid store API token".into()))?;
        let now = Utc::now();

        let record = self
            .repository
            .find_token_by_lookup_prefix(&token_prefix)
            .await?
            .ok_or_else(|| AppError::Unauthorized("Invalid store API token".into()))?;

        if record.revoked_at.is_some() || is_store_api_token_expired(record.expires_at, now) {
            return Err(AppError::Unauthorized("Invalid store API token".into()));
        }

        if !verify_secret(plaintext_token, &record.token_hash) {
            return Err(AppError::Unauthorized("Invalid store API token".into()));
        }

        self.repository.touch_last_used_at(record.id, now).await?;

        Ok(StoreApiTokenAuthContext {
            store_id: record.store_id,
            token_id: record.id,
            token_identifier: masked_display_prefix(&record.token_prefix),
        })
    }

    async fn ensure_store_exists(&self, store_id: Uuid) -> Result<(), AppError> {
        if self.repository.store_exists(store_id).await? {
            Ok(())
        } else {
            Err(AppError::NotFound("Store not found".into()))
        }
    }

    fn ensure_capability(
        &self,
        actor: &AuthenticatedUser,
        capability: Capability,
        store_id: Uuid,
    ) -> Result<(), AppError> {
        if has_capability(actor, capability, Some(store_id)) {
            Ok(())
        } else {
            Err(AppError::Forbidden(
                "You do not have permission to perform this action".into(),
            ))
        }
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
                target_type: Some("store_token".to_string()),
                target_id: Some(target_id),
                payload_json,
            })
            .await?;

        Ok(())
    }
}

fn map_metadata(record: StoreApiTokenRecord) -> StoreApiTokenMetadata {
    StoreApiTokenMetadata {
        id: record.id,
        name: record.name,
        display_prefix: masked_display_prefix(&record.token_prefix),
        last_used_at: record.last_used_at,
        expires_at: record.expires_at,
        created_at: record.created_at,
    }
}

fn generate_store_api_token() -> (String, String) {
    let mut lookup_bytes = [0_u8; STORE_API_TOKEN_LOOKUP_LENGTH / 2];
    rand::thread_rng().fill_bytes(&mut lookup_bytes);
    let lookup = lookup_bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let secret = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect::<String>();
    let token_prefix = format!("{STORE_API_TOKEN_PREFIX}{lookup}");
    let plaintext_token = format!("{token_prefix}.{secret}");

    (token_prefix, plaintext_token)
}

pub fn extract_lookup_prefix(plaintext_token: &str) -> Option<String> {
    let (token_prefix, secret) = plaintext_token.split_once('.')?;
    if secret.is_empty() || !token_prefix.starts_with(STORE_API_TOKEN_PREFIX) {
        return None;
    }

    let lookup = token_prefix.strip_prefix(STORE_API_TOKEN_PREFIX)?;
    if lookup.len() != STORE_API_TOKEN_LOOKUP_LENGTH
        || !lookup
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return None;
    }

    Some(token_prefix.to_string())
}

fn is_unique_prefix_conflict(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<SqlxError>()
        .and_then(|sqlx_error| match sqlx_error {
            SqlxError::Database(database_error) => {
                database_error.code().map(|code| code == "23505")
            }
            _ => None,
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;

    use super::*;
    use crate::shared::auth::{PlatformRole, StoreRole};

    #[derive(Default)]
    struct MockStoreTokenRepository {
        stores: Mutex<HashSet<Uuid>>,
        tokens: Mutex<HashMap<Uuid, StoreApiTokenRecord>>,
    }

    impl MockStoreTokenRepository {
        fn add_store(&self, store_id: Uuid) {
            self.stores.lock().unwrap().insert(store_id);
        }

        fn insert_existing_token(&self, token: StoreApiTokenRecord) {
            self.tokens.lock().unwrap().insert(token.id, token);
        }
    }

    #[async_trait]
    impl StoreTokenRepository for MockStoreTokenRepository {
        async fn store_exists(&self, store_id: Uuid) -> anyhow::Result<bool> {
            Ok(self.stores.lock().unwrap().contains(&store_id))
        }

        async fn list_active_tokens(
            &self,
            store_id: Uuid,
        ) -> anyhow::Result<Vec<StoreApiTokenRecord>> {
            let now = Utc::now();
            let mut tokens = self
                .tokens
                .lock()
                .unwrap()
                .values()
                .filter(|token| {
                    token.store_id == store_id
                        && token.revoked_at.is_none()
                        && !is_store_api_token_expired(token.expires_at, now)
                })
                .cloned()
                .collect::<Vec<_>>();
            tokens.sort_by(|left, right| right.created_at.cmp(&left.created_at));
            Ok(tokens)
        }

        async fn insert_token(
            &self,
            token: NewStoreApiTokenRecord,
        ) -> anyhow::Result<StoreApiTokenRecord> {
            let mut tokens = self.tokens.lock().unwrap();
            if tokens
                .values()
                .any(|existing| existing.token_prefix == token.token_prefix)
            {
                return Err(SqlxError::Protocol("duplicate token prefix".into()).into());
            }

            let record = StoreApiTokenRecord {
                id: token.id,
                store_id: token.store_id,
                name: token.name,
                token_prefix: token.token_prefix,
                token_hash: token.token_hash,
                last_used_at: None,
                expires_at: token.expires_at,
                revoked_at: None,
                created_by: token.created_by,
                created_at: token.created_at,
            };
            tokens.insert(record.id, record.clone());
            Ok(record)
        }

        async fn find_token_by_lookup_prefix(
            &self,
            token_prefix: &str,
        ) -> anyhow::Result<Option<StoreApiTokenRecord>> {
            Ok(self
                .tokens
                .lock()
                .unwrap()
                .values()
                .find(|token| token.token_prefix == token_prefix)
                .cloned())
        }

        async fn find_token_by_id(
            &self,
            store_id: Uuid,
            token_id: Uuid,
        ) -> anyhow::Result<Option<StoreApiTokenRecord>> {
            Ok(self
                .tokens
                .lock()
                .unwrap()
                .get(&token_id)
                .filter(|token| token.store_id == store_id)
                .cloned())
        }

        async fn mark_token_revoked(
            &self,
            store_id: Uuid,
            token_id: Uuid,
            revoked_at: chrono::DateTime<Utc>,
        ) -> anyhow::Result<bool> {
            let mut tokens = self.tokens.lock().unwrap();
            let Some(token) = tokens.get_mut(&token_id) else {
                return Ok(false);
            };
            if token.store_id != store_id || token.revoked_at.is_some() {
                return Ok(false);
            }

            token.revoked_at = Some(revoked_at);
            Ok(true)
        }

        async fn touch_last_used_at(
            &self,
            token_id: Uuid,
            last_used_at: chrono::DateTime<Utc>,
        ) -> anyhow::Result<()> {
            if let Some(token) = self.tokens.lock().unwrap().get_mut(&token_id) {
                token.last_used_at = Some(last_used_at);
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

    fn owner_actor(store_id: Uuid) -> AuthenticatedUser {
        AuthenticatedUser {
            user_id: Uuid::new_v4(),
            platform_role: PlatformRole::User,
            memberships: HashMap::from([(store_id, StoreRole::Owner)]),
        }
    }

    fn manager_actor(store_id: Uuid) -> AuthenticatedUser {
        AuthenticatedUser {
            user_id: Uuid::new_v4(),
            platform_role: PlatformRole::User,
            memberships: HashMap::from([(store_id, StoreRole::Manager)]),
        }
    }

    fn dev_actor() -> AuthenticatedUser {
        AuthenticatedUser {
            user_id: Uuid::new_v4(),
            platform_role: PlatformRole::Dev,
            memberships: HashMap::new(),
        }
    }

    fn admin_actor() -> AuthenticatedUser {
        AuthenticatedUser {
            user_id: Uuid::new_v4(),
            platform_role: PlatformRole::Admin,
            memberships: HashMap::new(),
        }
    }

    fn sample_token_record(
        store_id: Uuid,
        name: &str,
        lookup_prefix: &str,
        plaintext_token: &str,
    ) -> StoreApiTokenRecord {
        StoreApiTokenRecord {
            id: Uuid::new_v4(),
            store_id,
            name: name.to_string(),
            token_prefix: lookup_prefix.to_string(),
            token_hash: hash_secret(plaintext_token).unwrap(),
            last_used_at: None,
            expires_at: None,
            revoked_at: None,
            created_by: Uuid::new_v4(),
            created_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn create_token_returns_plaintext_once_and_persists_hash_only() {
        let store_id = Uuid::new_v4();
        let repository = Arc::new(MockStoreTokenRepository::default());
        repository.add_store(store_id);
        let audit_repository = Arc::new(MockAuditRepository::default());
        let service = StoreTokenService::new(repository.clone(), audit_repository.clone());
        let actor = owner_actor(store_id);

        let result = service
            .create_token(
                store_id,
                CreateStoreApiTokenRequest {
                    name: "Primary Token".into(),
                },
                &actor,
            )
            .await
            .unwrap();

        assert!(result.plaintext_token.starts_with(STORE_API_TOKEN_PREFIX));
        let lookup_prefix = extract_lookup_prefix(&result.plaintext_token).unwrap();
        assert_eq!(
            result.token.display_prefix,
            masked_display_prefix(&lookup_prefix)
        );
        assert_eq!(
            result.token.display_prefix.len(),
            STORE_API_TOKEN_PREFIX.len() + 8
        );

        let stored = repository
            .find_token_by_lookup_prefix(&lookup_prefix)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored.token_prefix, lookup_prefix);
        assert_ne!(stored.token_hash, result.plaintext_token);
        assert!(verify_secret(&result.plaintext_token, &stored.token_hash));

        let listed = service.list_tokens(store_id, &actor).await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].display_prefix, result.token.display_prefix);

        let entries = audit_repository.entries.lock().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].action, "store.token.create");
        let payload = entries[0].payload_json.to_string();
        assert!(!payload.contains(&result.plaintext_token));
        assert!(!payload.contains(&lookup_prefix));
    }

    #[tokio::test]
    async fn list_tokens_returns_only_active_safe_metadata() {
        let store_id = Uuid::new_v4();
        let repository = Arc::new(MockStoreTokenRepository::default());
        repository.add_store(store_id);
        let audit_repository = Arc::new(MockAuditRepository::default());
        let service = StoreTokenService::new(repository.clone(), audit_repository);
        let actor = owner_actor(store_id);

        let active_plaintext = "jq_sk_aaaaaaaaaaaa.secretactive";
        let revoked_plaintext = "jq_sk_bbbbbbbbbbbb.secretrevoked";
        let expired_plaintext = "jq_sk_cccccccccccc.secretexpired";

        let active = sample_token_record(
            store_id,
            "Active Token",
            "jq_sk_aaaaaaaaaaaa",
            active_plaintext,
        );
        let mut revoked = sample_token_record(
            store_id,
            "Revoked Token",
            "jq_sk_bbbbbbbbbbbb",
            revoked_plaintext,
        );
        revoked.revoked_at = Some(Utc::now());
        let mut expired = sample_token_record(
            store_id,
            "Expired Token",
            "jq_sk_cccccccccccc",
            expired_plaintext,
        );
        expired.expires_at = Some(Utc::now() - chrono::Duration::minutes(1));

        repository.insert_existing_token(active.clone());
        repository.insert_existing_token(revoked);
        repository.insert_existing_token(expired);

        let tokens = service.list_tokens(store_id, &actor).await.unwrap();

        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].name, "Active Token");
        assert_eq!(
            tokens[0].display_prefix,
            masked_display_prefix(&active.token_prefix)
        );
        assert_ne!(tokens[0].display_prefix, active.token_prefix);
        assert_eq!(tokens[0].display_prefix, "jq_sk_****aaaa");
    }

    #[tokio::test]
    async fn only_owner_and_dev_can_manage_tokens() {
        let store_id = Uuid::new_v4();
        let repository = Arc::new(MockStoreTokenRepository::default());
        repository.add_store(store_id);
        let audit_repository = Arc::new(MockAuditRepository::default());
        let service = StoreTokenService::new(repository, audit_repository);

        let owner = owner_actor(store_id);
        let dev = dev_actor();
        let manager = manager_actor(store_id);
        let admin = admin_actor();

        assert!(service
            .create_token(
                store_id,
                CreateStoreApiTokenRequest {
                    name: "Owner Token".into(),
                },
                &owner,
            )
            .await
            .is_ok());
        assert!(service
            .create_token(
                store_id,
                CreateStoreApiTokenRequest {
                    name: "Dev Token".into(),
                },
                &dev,
            )
            .await
            .is_ok());

        let manager_error = service
            .create_token(
                store_id,
                CreateStoreApiTokenRequest {
                    name: "Manager Token".into(),
                },
                &manager,
            )
            .await
            .unwrap_err();
        assert!(matches!(manager_error, AppError::Forbidden(_)));

        let admin_error = service.list_tokens(store_id, &admin).await.unwrap_err();
        assert!(matches!(admin_error, AppError::Forbidden(_)));
    }

    #[tokio::test]
    async fn revoke_is_idempotent_and_missing_token_is_not_found() {
        let store_id = Uuid::new_v4();
        let repository = Arc::new(MockStoreTokenRepository::default());
        repository.add_store(store_id);
        let audit_repository = Arc::new(MockAuditRepository::default());
        let service = StoreTokenService::new(repository.clone(), audit_repository.clone());
        let actor = owner_actor(store_id);

        let token = sample_token_record(
            store_id,
            "Revocable Token",
            "jq_sk_dddddddddddd",
            "jq_sk_dddddddddddd.secretrevocable",
        );
        let token_id = token.id;
        repository.insert_existing_token(token);

        let first_revoke = service
            .revoke_token(store_id, token_id, &actor)
            .await
            .unwrap();
        let second_revoke = service
            .revoke_token(store_id, token_id, &actor)
            .await
            .unwrap();

        assert_eq!(first_revoke, RevokeStoreApiTokenResult::Revoked);
        assert_eq!(second_revoke, RevokeStoreApiTokenResult::AlreadyRevoked);
        assert!(repository
            .find_token_by_id(store_id, token_id)
            .await
            .unwrap()
            .unwrap()
            .revoked_at
            .is_some());

        let entries = audit_repository.entries.lock().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].action, "store.token.revoke");
        drop(entries);

        let missing_error = service
            .revoke_token(store_id, Uuid::new_v4(), &actor)
            .await
            .unwrap_err();
        assert!(matches!(missing_error, AppError::NotFound(_)));
    }

    #[tokio::test]
    async fn resolve_bearer_token_updates_last_used_and_rejects_revoked_tokens() {
        let store_id = Uuid::new_v4();
        let repository = Arc::new(MockStoreTokenRepository::default());
        repository.add_store(store_id);
        let audit_repository = Arc::new(MockAuditRepository::default());
        let service = StoreTokenService::new(repository.clone(), audit_repository);
        let actor = owner_actor(store_id);

        let created = service
            .create_token(
                store_id,
                CreateStoreApiTokenRequest {
                    name: "Resolve Token".into(),
                },
                &actor,
            )
            .await
            .unwrap();

        let context = service
            .resolve_bearer_token(&created.plaintext_token)
            .await
            .unwrap();
        assert_eq!(context.store_id, store_id);
        assert_eq!(context.token_identifier, created.token.display_prefix);

        let lookup_prefix = extract_lookup_prefix(&created.plaintext_token).unwrap();
        let stored = repository
            .find_token_by_lookup_prefix(&lookup_prefix)
            .await
            .unwrap()
            .unwrap();
        assert!(stored.last_used_at.is_some());

        service
            .revoke_token(store_id, stored.id, &actor)
            .await
            .unwrap();

        let error = service
            .resolve_bearer_token(&created.plaintext_token)
            .await
            .unwrap_err();
        assert!(matches!(error, AppError::Unauthorized(_)));
    }
}
